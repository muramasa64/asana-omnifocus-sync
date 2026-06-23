// JXA: stdin の操作 JSON を OmniFocus に適用する。
// 入力: { project, tag_root, operations: [ {kind:"create"|"update"|"complete", ...} ] }
// 出力: { created, updated, completed }

function readStdin() {
  // ObjC ブリッジで標準入力を全読みする。
  ObjC.import("Foundation");
  const handle = $.NSFileHandle.fileHandleWithStandardInput;
  const data = handle.readDataToEndOfFile;
  return $.NSString.alloc.initWithDataEncoding(data, $.NSUTF8StringEncoding).js;
}

// "YYYY-MM-DD" をローカル時刻の Date（正午）に変換する。
// 正午にするのは、タイムゾーン差で前日/翌日へずれるのを避けるため。
function parseDue(s) {
  if (!s) return null;
  const parts = s.split("-");
  if (parts.length < 3) return null;
  return new Date(
    parseInt(parts[0], 10),
    parseInt(parts[1], 10) - 1,
    parseInt(parts[2], 10),
    12, 0, 0
  );
}

function run() {
  const of = Application("OmniFocus");
  const doc = of.defaultDocument;
  const input = JSON.parse(readStdin());

  // 取り込み先プロジェクトを取得（無ければ作成）。
  let projects = doc.flattenedProjects.whose({ name: input.project })();
  let project;
  if (projects.length === 0) {
    project = of.Project({ name: input.project });
    doc.projects.push(project);
  } else {
    project = projects[0];
  }

  // ルートタグを取得（無ければトップレベルに作成）。
  let roots = doc.flattenedTags.whose({ name: input.tag_root })();
  let rootTag;
  if (roots.length === 0) {
    rootTag = of.Tag({ name: input.tag_root });
    doc.tags.push(rootTag);
  } else {
    rootTag = roots[0];
  }

  // ルートタグ配下の子タグを名前で取得（無ければ作成）。
  function childTag(name) {
    const existing = rootTag.tags.whose({ name: name })();
    if (existing.length > 0) return existing[0];
    const t = of.Tag({ name: name });
    rootTag.tags.push(t);
    return t;
  }

  // タスクの管理対象タグ（ルートタグ配下）を names に置き換える。
  // ルートタグ配下にないタグ（利用者の手動タグ）は触らない。
  function setTags(task, names) {
    const childIds = {};
    const children = rootTag.tags();
    for (let i = 0; i < children.length; i++) {
      childIds[children[i].id()] = true;
    }
    const assigned = task.tags();
    for (let i = 0; i < assigned.length; i++) {
      if (childIds[assigned[i].id()]) {
        of.remove(assigned[i], { from: task.tags });
      }
    }
    if (names.length === 0) {
      of.add(rootTag, { to: task.tags });
    } else {
      for (let i = 0; i < names.length; i++) {
        of.add(childTag(names[i]), { to: task.tags });
      }
    }
  }

  // of_id からタスクを引くための索引。
  function taskById(id) {
    const found = doc.flattenedTasks.whose({ id: id })();
    return found.length > 0 ? found[0] : null;
  }

  let created = 0, updated = 0, completed = 0;

  for (let i = 0; i < input.operations.length; i++) {
    const op = input.operations[i];
    if (op.kind === "create") {
      const task = of.Task({ name: op.name, note: op.note });
      project.tasks.push(task);
      const due = parseDue(op.due);
      if (due) task.dueDate = due;
      setTags(task, op.tags || []);
      created++;
    } else if (op.kind === "update") {
      const task = taskById(op.of_id);
      if (task) {
        task.name = op.name;
        task.note = op.note;
        task.dueDate = parseDue(op.due); // null で due 解除
        setTags(task, op.tags || []);
        updated++;
      }
    } else if (op.kind === "complete") {
      const task = taskById(op.of_id);
      if (task) {
        of.markComplete(task);
        completed++;
      }
    }
  }

  return JSON.stringify({ created: created, updated: updated, completed: completed });
}
