import { Database, ScanSearch, ShieldCheck } from "lucide-react";

const foundations = [
  {
    icon: ShieldCheck,
    title: "決定論で最終判定",
    description: "Codexは調査と抽出を担当し、採用判定はアプリ内の検証ロジックで行います。",
  },
  {
    icon: Database,
    title: "Ver.7.0カタログ",
    description: "キャラクター・武器・聖遺物を独自Schemaへ変換し、整合性を検証します。",
  },
  {
    icon: ScanSearch,
    title: "根拠を追跡",
    description: "推奨内容と出典を紐付け、根拠不足や条件の衝突を見える形で扱います。",
  },
];

function App() {
  return (
    <main className="min-h-dvh bg-slate-950 px-6 py-12 text-slate-100 sm:px-10">
      <section className="mx-auto flex min-h-[calc(100dvh-6rem)] max-w-5xl flex-col justify-center">
        <p className="mb-3 text-sm font-semibold text-amber-400">原神 Ver.7.0 対応</p>
        <h1 className="max-w-3xl text-balance text-4xl font-bold leading-tight sm:text-5xl">
          根拠付き・編成連動ビルド推薦
        </h1>
        <p className="mt-5 max-w-2xl text-pretty text-lg leading-8 text-slate-300">
          4人編成、武器、精錬、命ノ星座、役割をもとに、条件へ適合する聖遺物候補を比較します。
          現在は安全な分析基盤を準備中です。
        </p>

        <div className="mt-10 grid gap-4 md:grid-cols-3">
          {foundations.map(({ icon: Icon, title, description }) => (
            <article key={title} className="rounded-xl border border-slate-800 bg-slate-900 p-5 shadow-sm">
              <Icon aria-hidden="true" className="size-6 text-amber-400" />
              <h2 className="mt-4 text-balance text-lg font-semibold">{title}</h2>
              <p className="mt-2 text-pretty leading-6 text-slate-400">{description}</p>
            </article>
          ))}
        </div>

        <p className="mt-8 text-sm text-slate-500" role="status">
          Gate 0: Codex App Server接続の技術検証を実行します。
        </p>
      </section>
    </main>
  );
}

export default App;
