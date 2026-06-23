// JXA: 取り込み先プロジェクト配下で note に `asana_gid:` を持つタスクを JSON 出力する。
// 引数1: プロジェクト名。引数2: ルートタグ名。stdout に OfTask 配列を出力。
function run(argv) {
  const projectName = argv[0];
  const tagRoot = argv[1];
  const of = Application("OmniFocus");
  const doc = of.defaultDocument;

  const projects = doc.flattenedProjects.whose({ name: projectName })();
  if (projects.length === 0) {
    return "[]";
  }
  const project = projects[0];

  // ルートタグ配下の子タグ id 集合（管理対象タグの判定に使う）。
  const managedTagIds = {};
  const roots = doc.flattenedTags.whose({ name: tagRoot })();
  if (roots.length > 0) {
    const children = roots[0].tags();
    for (let i = 0; i < children.length; i++) {
      managedTagIds[children[i].id()] = children[i].name();
    }
  }

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

    // 付与タグのうち、ルートタグ配下のもの（管理対象タグ）の名前だけを採る。
    const tags = [];
    const assigned = t.tags();
    for (let j = 0; j < assigned.length; j++) {
      const name = managedTagIds[assigned[j].id()];
      if (name) tags.push(name);
    }

    out.push({
      of_id: t.id(),
      asana_gid: m[1],
      name: t.name(),
      due: localDate(t.dueDate()),
      completed: t.completed(),
      note: note,
      tags: tags,
    });
  }
  return JSON.stringify(out);
}
