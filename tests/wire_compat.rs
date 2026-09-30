use capnp::schema_capnp::{code_generator_request, field, node, type_, value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const LINKS: &[&str] = &["windows.capnp", "linux.capnp"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Unchanged,
    Additive,
    Breaking,
}

#[derive(Default)]
struct Changes(Vec<(Level, String)>);

impl Changes {
    fn level(&self) -> Level {
        self.0.iter().map(|(level, _)| *level).max().unwrap_or(Level::Unchanged)
    }

    fn additive(&mut self, what: String) {
        self.0.push((Level::Additive, what));
    }

    fn breaking(&mut self, what: String) {
        self.0.push((Level::Breaking, what));
    }
}

type Version = [u32; 3];

struct Compiled(capnp::message::Reader<capnp::serialize::OwnedSegments>);

fn compile(dir: &Path) -> Compiled {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("schema dir")
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "capnp"))
        .collect();
    files.sort();
    let output = Command::new("capnp")
        .arg("compile")
        .arg("-o-")
        .arg(format!("--src-prefix={}", dir.display()))
        .args(&files)
        .output()
        .expect("failed to run `capnp compile`; is the `capnp` binary installed?");
    assert!(
        output.status.success(),
        "`capnp compile` failed in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    Compiled(
        capnp::serialize::read_message(&mut output.stdout.as_slice(), capnp::message::ReaderOptions::new())
            .expect("code generator request"),
    )
}

struct Schema<'a> {
    nodes: HashMap<u64, node::Reader<'a>>,
    files: HashMap<String, u64>,
}

impl<'a> Schema<'a> {
    fn new(compiled: &'a Compiled) -> Self {
        let request = compiled
            .0
            .get_root::<code_generator_request::Reader>()
            .expect("request root");
        let nodes = request
            .get_nodes()
            .expect("nodes")
            .iter()
            .map(|node| (node.get_id(), node))
            .collect();
        let files = request
            .get_requested_files()
            .expect("requested files")
            .iter()
            .map(|file| (file.get_filename().unwrap().to_string().unwrap(), file.get_id()))
            .collect();
        Self { nodes, files }
    }

    fn node(&self, id: u64) -> node::Reader<'a> {
        *self.nodes.get(&id).unwrap_or_else(|| panic!("node {id:#x} is not in the request"))
    }

    fn name(&self, id: u64) -> String {
        self.node(id).get_display_name().unwrap().to_string().unwrap()
    }

    fn version(&self, file: &str) -> Option<Version> {
        let wanted = format!("{file}:version");
        let node = self.nodes.values().find(|node| {
            node.get_display_name().map(|n| n == wanted.as_str()).unwrap_or(false)
        })?;
        let node::Const(constant) = node.which().unwrap() else {
            panic!("{wanted} is not a const");
        };
        let value::Text(text) = constant.get_value().unwrap().which().unwrap() else {
            panic!("{wanted} is not Text");
        };
        let text = text.unwrap().to_string().unwrap();
        let parts: Vec<u32> = text.split('.').map(|p| p.parse().unwrap()).collect();
        Some(parts.try_into().unwrap_or_else(|_| panic!("{wanted} = {text:?}")))
    }
}

struct Diff<'s, 'a> {
    old: &'s Schema<'a>,
    new: &'s Schema<'a>,
    seen: HashSet<(u64, u64)>,
    changes: Changes,
}

impl<'s, 'a> Diff<'s, 'a> {
    fn link(old: &'s Schema<'a>, new: &'s Schema<'a>, file: &str) -> Changes {
        let mut diff = Diff {
            old,
            new,
            seen: HashSet::new(),
            changes: Changes::default(),
        };
        let old_file = old.files[file];
        let Some(&new_file) = new.files.get(file) else {
            diff.changes.breaking(format!("{file} is gone"));
            return diff.changes;
        };
        if old_file != new_file {
            diff.changes.breaking(format!(
                "{file}: file id {old_file:#x} became {new_file:#x}; it is the protocol id"
            ));
        }
        for nested in old.node(old_file).get_nested_nodes().unwrap() {
            let id = nested.get_id();
            if !matches!(old.node(id).which().unwrap(), node::Interface(_)) {
                continue;
            }
            if new.nodes.contains_key(&id) {
                diff.interface(id, id);
            } else {
                diff.changes.breaking(format!("interface {} is gone", old.name(id)));
            }
        }
        diff.changes
    }

    fn interface(&mut self, old_id: u64, new_id: u64) {
        let path = self.new.name(new_id);
        if old_id != new_id {
            self.changes.breaking(format!(
                "{} became {path}: the interface id travels in every call",
                self.old.name(old_id)
            ));
            return;
        }
        if !self.seen.insert((old_id, new_id)) {
            return;
        }
        let node::Interface(old) = self.old.node(old_id).which().unwrap() else {
            unreachable!()
        };
        let node::Interface(new) = self.new.node(new_id).which().unwrap() else {
            self.changes.breaking(format!("{path} is no longer an interface"));
            return;
        };

        let supers = |list: capnp::struct_list::Reader<'a, capnp::schema_capnp::superclass::Owned>| {
            list.iter().map(|s| s.get_id()).collect::<Vec<_>>()
        };
        if supers(old.get_superclasses().unwrap()) != supers(new.get_superclasses().unwrap()) {
            self.changes.breaking(format!("{path}: superclasses changed"));
        }

        let old_methods = old.get_methods().unwrap();
        let new_methods = new.get_methods().unwrap();
        let names = |list: capnp::struct_list::Reader<'a, capnp::schema_capnp::method::Owned>| {
            list.iter()
                .map(|m| m.get_name().unwrap().to_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        moved_names(&names(old_methods), &names(new_methods), &path, &mut self.changes);
        for (index, old_method) in old_methods.iter().enumerate() {
            let Some(new_method) = new_methods.try_get(index as u32) else {
                self.changes.breaking(format!(
                    "{path}.{} @{index} is gone",
                    old_method.get_name().unwrap().to_str().unwrap()
                ));
                continue;
            };
            let name = format!("{path}.{}", new_method.get_name().unwrap().to_str().unwrap());
            self.structure(
                old_method.get_param_struct_type(),
                new_method.get_param_struct_type(),
                &format!("{name}(params)"),
            );
            self.structure(
                old_method.get_result_struct_type(),
                new_method.get_result_struct_type(),
                &format!("{name}(results)"),
            );
        }
        for new_method in new_methods.iter().skip(old_methods.len() as usize) {
            self.changes.additive(format!(
                "{path}.{} added",
                new_method.get_name().unwrap().to_str().unwrap()
            ));
        }
    }

    fn structure(&mut self, old_id: u64, new_id: u64, path: &str) {
        if !self.seen.insert((old_id, new_id)) {
            return;
        }
        let node::Struct(old) = self.old.node(old_id).which().unwrap() else {
            unreachable!()
        };
        let node::Struct(new) = self.new.node(new_id).which().unwrap() else {
            self.changes.breaking(format!("{path} is no longer a struct"));
            return;
        };

        if new.get_data_word_count() < old.get_data_word_count()
            || new.get_pointer_count() < old.get_pointer_count()
        {
            self.changes.breaking(format!("{path}: layout shrank"));
        }

        let (old_count, new_count) = (old.get_discriminant_count(), new.get_discriminant_count());
        if old_count != 0 || new_count != 0 {
            let same_tag = old.get_discriminant_offset() == new.get_discriminant_offset();
            if old_count == 0 || !same_tag || new_count < old_count {
                self.changes.breaking(format!(
                    "{path}: union layout changed (adding a union to an existing struct is not analysed)"
                ));
            } else if new_count > old_count {
                self.changes.additive(format!("{path}: union members added"));
            }
        }

        let new_fields: Vec<field::Reader<'a>> = new.get_fields().unwrap().iter().collect();
        let mut matched = HashSet::new();
        for old_field in old.get_fields().unwrap() {
            let name = old_field.get_name().unwrap().to_str().unwrap();
            let found = match old_field.get_ordinal().which().unwrap() {
                field::ordinal::Explicit(ordinal) => new_fields.iter().position(|f| {
                    matches!(f.get_ordinal().which().unwrap(), field::ordinal::Explicit(o) if o == ordinal)
                }),
                field::ordinal::Implicit(()) => new_fields
                    .iter()
                    .position(|f| f.get_name().unwrap().to_str().unwrap() == name),
            };
            let Some(index) = found else {
                self.changes.breaking(format!("{path}.{name} is gone"));
                continue;
            };
            let now_there = new_fields[index].get_name().unwrap().to_str().unwrap();
            if now_there != name
                && new_fields.iter().any(|f| f.get_name().unwrap().to_str().unwrap() == name)
            {
                self.changes.breaking(format!(
                    "{path}.{name} moved to another ordinal; its old one is now {now_there}"
                ));
            }
            matched.insert(index);
            self.field(old_field, new_fields[index], &format!("{path}.{name}"));
        }
        for (index, new_field) in new_fields.iter().enumerate() {
            if !matched.contains(&index) {
                self.changes.additive(format!(
                    "{path}.{} added",
                    new_field.get_name().unwrap().to_str().unwrap()
                ));
            }
        }
    }

    fn field(&mut self, old: field::Reader<'a>, new: field::Reader<'a>, path: &str) {
        if old.get_discriminant_value() != new.get_discriminant_value() {
            self.changes.breaking(format!("{path}: moved in or out of a union"));
        }
        match (old.which().unwrap(), new.which().unwrap()) {
            (field::Slot(old), field::Slot(new)) => {
                if old.get_offset() != new.get_offset() {
                    self.changes.breaking(format!("{path}: offset moved"));
                }
                self.kind(old.get_type().unwrap(), new.get_type().unwrap(), path);
                self.default(old, new, path);
            }
            (field::Group(old), field::Group(new)) => {
                self.structure(old.get_type_id(), new.get_type_id(), path);
            }
            _ => self.changes.breaking(format!("{path}: switched between field and group")),
        }
    }

    fn kind(&mut self, old: type_::Reader<'a>, new: type_::Reader<'a>, path: &str) {
        use type_::Which as T;
        match (old.which().unwrap(), new.which().unwrap()) {
            (T::List(old), T::List(new)) => self.kind(
                old.get_element_type().unwrap(),
                new.get_element_type().unwrap(),
                &format!("{path}[]"),
            ),
            (T::Struct(old), T::Struct(new)) => {
                self.structure(old.get_type_id(), new.get_type_id(), path)
            }
            (T::Enum(old), T::Enum(new)) => self.enumeration(old.get_type_id(), new.get_type_id(), path),
            (T::Interface(old), T::Interface(new)) => {
                self.interface(old.get_type_id(), new.get_type_id())
            }
            (old_kind, new_kind) => {
                let (old_name, new_name) = (scalar(&old_kind), scalar(&new_kind));
                if old_name.is_none() || old_name != new_name {
                    self.changes.breaking(format!("{path}: type changed"));
                }
            }
        }
    }

    fn enumeration(&mut self, old_id: u64, new_id: u64, path: &str) {
        let node::Enum(old) = self.old.node(old_id).which().unwrap() else {
            unreachable!()
        };
        let node::Enum(new) = self.new.node(new_id).which().unwrap() else {
            self.changes.breaking(format!("{path}: no longer an enum"));
            return;
        };
        if !self.seen.insert((old_id, new_id)) {
            return;
        }
        let names = |list: capnp::struct_list::Reader<'a, capnp::schema_capnp::enumerant::Owned>| {
            list.iter()
                .map(|e| e.get_name().unwrap().to_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        let (old_names, new_names) = (names(old.get_enumerants().unwrap()), names(new.get_enumerants().unwrap()));
        let enum_path = self.new.name(new_id);
        moved_names(&old_names, &new_names, &enum_path, &mut self.changes);
        if new_names.len() < old_names.len() {
            self.changes.breaking(format!("{enum_path}: enumerants removed"));
        } else if new_names.len() > old_names.len() {
            self.changes.additive(format!("{enum_path}: enumerants added"));
        }
    }

    fn default(&mut self, old: field::slot::Reader<'a>, new: field::slot::Reader<'a>, path: &str) {
        use value::Which as V;
        let (old_value, new_value) = (old.get_default_value().unwrap(), new.get_default_value().unwrap());
        let same = match (old_value.which().unwrap(), new_value.which().unwrap()) {
            (V::Void(()), V::Void(())) => true,
            (V::Bool(a), V::Bool(b)) => a == b,
            (V::Int8(a), V::Int8(b)) => a == b,
            (V::Int16(a), V::Int16(b)) => a == b,
            (V::Int32(a), V::Int32(b)) => a == b,
            (V::Int64(a), V::Int64(b)) => a == b,
            (V::Uint8(a), V::Uint8(b)) => a == b,
            (V::Uint16(a), V::Uint16(b)) => a == b,
            (V::Uint32(a), V::Uint32(b)) => a == b,
            (V::Uint64(a), V::Uint64(b)) => a == b,
            (V::Float32(a), V::Float32(b)) => a.to_bits() == b.to_bits(),
            (V::Float64(a), V::Float64(b)) => a.to_bits() == b.to_bits(),
            (V::Enum(a), V::Enum(b)) => a == b,
            (V::Text(a), V::Text(b)) => a.unwrap().as_bytes() == b.unwrap().as_bytes(),
            (V::Data(a), V::Data(b)) => a.unwrap() == b.unwrap(),
            (V::Interface(()), V::Interface(())) => true,
            _ if !old.get_had_explicit_default() && !new.get_had_explicit_default() => true,
            _ => {
                self.changes.breaking(format!(
                    "{path}: explicit defaults on struct, list and AnyPointer fields are not analysed"
                ));
                return;
            }
        };
        if !same {
            self.changes.breaking(format!("{path}: default value changed"));
        }
    }
}

fn moved_names(old: &[String], new: &[String], owner: &str, changes: &mut Changes) {
    for (ordinal, name) in old.iter().enumerate() {
        if let Some(now) = new.iter().position(|n| n == name)
            && now != ordinal
        {
            changes.breaking(format!("{owner}.{name} moved from @{ordinal} to @{now}"));
        }
    }
}

fn scalar(kind: &type_::WhichReader) -> Option<&'static str> {
    use type_::Which as T;
    Some(match kind {
        T::Void(()) => "Void",
        T::Bool(()) => "Bool",
        T::Int8(()) => "Int8",
        T::Int16(()) => "Int16",
        T::Int32(()) => "Int32",
        T::Int64(()) => "Int64",
        T::Uint8(()) => "UInt8",
        T::Uint16(()) => "UInt16",
        T::Uint32(()) => "UInt32",
        T::Uint64(()) => "UInt64",
        T::Float32(()) => "Float32",
        T::Float64(()) => "Float64",
        T::Text(()) => "Text",
        T::Data(()) => "Data",
        T::AnyPointer(_) => "AnyPointer",
        _ => return None,
    })
}

fn verdict(level: Level, old: Option<Version>, new: Option<Version>) -> Result<(), String> {
    let Some(old) = old else {
        return Ok(());
    };
    let Some(new) = new else {
        return Err("the schema no longer declares a version".into());
    };
    let as_text = |v: Version| format!("{}.{}.{}", v[0], v[1], v[2]);
    if new < old {
        return Err(format!("version went back from {} to {}", as_text(old), as_text(new)));
    }
    let enough = match level {
        Level::Unchanged => true,
        Level::Additive => new[0] > old[0] || new[1] > old[1],
        Level::Breaking => new[0] > old[0],
    };
    if enough {
        Ok(())
    } else {
        Err(format!(
            "{level:?} changes need a {} bump, but the version went from {} to {}",
            if level == Level::Breaking { "major" } else { "minor" },
            as_text(old),
            as_text(new)
        ))
    }
}

fn scratch_dir(label: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "uniproc-protocol-compat-{}-{}-{label}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn git(args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("git must be on PATH");
    assert!(
        output.status.success(),
        "git {} failed: {}\nThis test needs the full history and tags: actions/checkout with fetch-depth: 0 in CI, `git fetch --unshallow --tags` in a shallow clone.",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git output is utf-8")
}

#[test]
fn every_wire_change_since_the_last_tag_is_versioned() {
    let at_head = git(&["tag", "--points-at", "HEAD"]);
    let mut describe = vec!["describe", "--tags", "--abbrev=0"];
    for own in at_head.lines() {
        describe.extend(["--exclude", own]);
    }
    describe.push("HEAD");
    let tag = git(&describe).trim().to_string();
    let old_dir = scratch_dir("tag");
    for path in git(&["ls-tree", "--name-only", &tag, "schema/"]).lines() {
        let text = git(&["show", &format!("{tag}:{path}")]);
        let name = Path::new(path).file_name().unwrap();
        std::fs::write(old_dir.join(name), text).unwrap();
    }
    let new_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema");

    let (old_compiled, new_compiled) = (compile(&old_dir), compile(&new_dir));
    let (old, new) = (Schema::new(&old_compiled), Schema::new(&new_compiled));

    let mut failures = Vec::new();
    for link in LINKS {
        if !old.files.contains_key(*link) {
            continue;
        }
        let changes = Diff::link(&old, &new, link);
        let (old_version, new_version) = (old.version(link), new.version(link));
        eprintln!("{link} since {tag}: {:?}, {old_version:?} -> {new_version:?}", changes.level());
        for (level, what) in &changes.0 {
            eprintln!("  {level:?}: {what}");
        }
        if let Err(reason) = verdict(changes.level(), old_version, new_version) {
            failures.push(format!("{link}: {reason}"));
        }
    }
    let _ = std::fs::remove_dir_all(&old_dir);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

const BASE: &str = r#"@0xb1f0c2d3e4f50617;
interface Svc {
  a @0 (x :UInt32) -> (y :Text);
  b @1 (s :S) -> (e :E);
}
struct S {
  p @0 :UInt32;
  q @1 :Text;
}
enum E {
  one @0;
  two @1;
}
"#;

fn level_between(old: &str, new: &str) -> Level {
    let old_dir = scratch_dir("old");
    let new_dir = scratch_dir("new");
    std::fs::write(old_dir.join("x.capnp"), old).unwrap();
    std::fs::write(new_dir.join("x.capnp"), new).unwrap();
    let (old_compiled, new_compiled) = (compile(&old_dir), compile(&new_dir));
    let level = Diff::link(&Schema::new(&old_compiled), &Schema::new(&new_compiled), "x.capnp").level();
    let _ = std::fs::remove_dir_all(&old_dir);
    let _ = std::fs::remove_dir_all(&new_dir);
    level
}

fn edited(from: &str, to: &str) -> String {
    assert!(BASE.contains(from), "{from:?} is not in the base schema");
    BASE.replacen(from, to, 1)
}

#[test]
fn comments_and_renames_of_members_do_not_touch_the_wire() {
    let new = edited("  a @0 (x :UInt32) -> (y :Text);", "  # doc\n  alpha @0 (renamed :UInt32) -> (out :Text);");
    assert_eq!(level_between(BASE, &new), Level::Unchanged);
}

#[test]
fn a_renamed_struct_is_followed_through_the_method_that_uses_it() {
    let new = BASE
        .replace("(s :S)", "(s :Renamed)")
        .replace("struct S {", "struct Renamed {");
    assert_eq!(level_between(BASE, &new), Level::Unchanged);
}

#[test]
fn appending_is_additive() {
    for new in [
        edited("  b @1 (s :S) -> (e :E);", "  b @1 (s :S) -> (e :E);\n  c @2 () -> ();"),
        edited("  q @1 :Text;", "  q @1 :Text;\n  r @2 :Bool;"),
        edited("  two @1;", "  two @1;\n  three @2;"),
        edited("(x :UInt32) -> (y :Text)", "(x :UInt32, z :Data) -> (y :Text)"),
    ] {
        assert_eq!(level_between(BASE, &new), Level::Additive, "{new}");
    }
}

#[test]
fn anything_else_on_the_wire_is_breaking() {
    for new in [
        edited("  q @1 :Text;", "  q @1 :Data;"),
        edited("  p @0 :UInt32;\n  q @1 :Text;", "  p @1 :UInt32;\n  q @0 :Text;"),
        edited("  b @1 (s :S) -> (e :E);\n", ""),
        edited("interface Svc {", "interface Renamed {"),
        edited("@0xb1f0c2d3e4f50617;", "@0xb1f0c2d3e4f50618;"),
        edited("  two @1;\n", ""),
        edited("  p @0 :UInt32;", "  p @0 :UInt32 = 7;"),
        edited("(x :UInt32)", "(x :UInt64)"),
    ] {
        assert_eq!(level_between(BASE, &new), Level::Breaking, "{new}");
    }
}

#[test]
fn an_enumerant_inserted_before_others_is_breaking() {
    let new = edited("  one @0;\n  two @1;", "  one @0;\n  between @1;\n  two @2;");
    assert_eq!(level_between(BASE, &new), Level::Breaking);
}

#[test]
fn fields_of_one_type_swapping_names_is_breaking() {
    let old = BASE.replace("  q @1 :Text;", "  q @1 :Text;\n  rx @2 :UInt64;\n  tx @3 :UInt64;");
    let new = BASE.replace("  q @1 :Text;", "  q @1 :Text;\n  tx @2 :UInt64;\n  rx @3 :UInt64;");
    assert_eq!(level_between(&old, &new), Level::Breaking);
}

#[test]
fn the_version_must_rise_with_the_change() {
    assert!(verdict(Level::Unchanged, Some([1, 2, 0]), Some([1, 2, 0])).is_ok());
    assert!(verdict(Level::Additive, Some([1, 2, 0]), Some([1, 3, 0])).is_ok());
    assert!(verdict(Level::Additive, Some([1, 2, 0]), Some([1, 2, 1])).is_err());
    assert!(verdict(Level::Breaking, Some([1, 2, 0]), Some([1, 9, 0])).is_err());
    assert!(verdict(Level::Breaking, Some([1, 2, 0]), Some([2, 0, 0])).is_ok());
    assert!(verdict(Level::Unchanged, Some([1, 2, 0]), Some([1, 1, 9])).is_err());
    assert!(verdict(Level::Breaking, None, Some([1, 0, 0])).is_ok());
}
