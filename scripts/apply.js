// JXA: stdin の操作 JSON を OmniFocus に適用する。
// 入力: { project, operations: [ {kind:"create"|"update"|"complete", ...} ] }
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
      created++;
    } else if (op.kind === "update") {
      const task = taskById(op.of_id);
      if (task) {
        task.name = op.name;
        task.note = op.note;
        task.dueDate = parseDue(op.due); // null で due 解除
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
