// JXA: 取り込み先プロジェクト配下で note に `asana_gid:` を持つタスクを JSON 出力する。
// 引数1: プロジェクト名。stdout に OfTask 配列を出力。
function run(argv) {
  const projectName = argv[0];
  const of = Application("OmniFocus");
  const doc = of.defaultDocument;

  const projects = doc.flattenedProjects.whose({ name: projectName })();
  if (projects.length === 0) {
    return "[]";
  }
  const project = projects[0];

  // due は日付粒度（ローカルの YYYY-MM-DD）で出力し、Asana 側と比較を安定させる。
  function localDate(d) {
    if (!d) return null;
    const y = d.getFullYear();
    const m = ("0" + (d.getMonth() + 1)).slice(-2);
    const day = ("0" + d.getDate()).slice(-2);
    return y + "-" + m + "-" + day;
  }

  const gidRe = /^asana_gid:\s*(\S+)/m;
  const out = [];
  const tasks = project.flattenedTasks();
  for (let i = 0; i < tasks.length; i++) {
    const t = tasks[i];
    const note = t.note() || "";
    const m = note.match(gidRe);
    if (!m) continue;

    out.push({
      of_id: t.id(),
      asana_gid: m[1],
      name: t.name(),
      due: localDate(t.dueDate()),
      completed: t.completed(),
      note: note,
    });
  }
  return JSON.stringify(out);
}
