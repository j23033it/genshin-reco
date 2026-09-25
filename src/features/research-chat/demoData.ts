import type { TeamMember } from "./types";

export const DEMO_TEAM: TeamMember[] = [
  {
    id: "arlecchino",
    name: "アルレッキーノ",
    element: "炎",
    role: "メインアタッカー",
    constellation: "無凸前提",
    imageUrl: "https://gi.yatta.moe/assets/UI/UI_AvatarIcon_Arlecchino.png",
    weapon: "赤月のシルエット",
    weaponImageUrl:
      "https://gi.yatta.moe/assets/UI/UI_EquipIcon_Pole_BloodMoon.png",
    artifact: "諧律奇想の断章",
    artifactImageUrl:
      "https://gi.yatta.moe/assets/UI/reliquary/UI_RelicIcon_15035_4.png",
    mainStats: "攻撃力% / 炎元素ダメージ / 会心",
    subStats: "会心率 ＞ 会心ダメージ ＞ 攻撃力%",
    targetStats: [
      { label: "攻撃力", value: "2,000–2,300", primary: true },
      { label: "会心率", value: "70–80%", note: "デモ値。戦闘中に会心率が加算される場合は、合計が100%を超えないよう戦闘前の値を調整。" },
      { label: "会心ダメージ", value: "180%以上" },
      { label: "元素熟知", value: "100–150" },
      { label: "炎元素ダメージ", value: "46.6%" },
    ],
  },
  {
    id: "yelan",
    name: "夜蘭",
    element: "水",
    role: "サブアタッカー",
    constellation: "無凸前提",
    imageUrl: "https://gi.yatta.moe/assets/UI/UI_AvatarIcon_Yelan.png",
    weapon: "若水",
    weaponImageUrl: "https://gi.yatta.moe/assets/UI/UI_EquipIcon_Bow_Kirin.png",
    artifact: "絶縁の旗印",
    artifactImageUrl:
      "https://gi.yatta.moe/assets/UI/reliquary/UI_RelicIcon_15020_4.png",
    mainStats: "元素チャージ / 水元素ダメージ / 会心",
    subStats: "元素チャージ ＞ 会心率 ＞ 会心ダメージ",
    targetStats: [
      { label: "HP", value: "32,000–36,000", primary: true, note: "デモ値。編成効果や固有天賦で変わる値は、発動条件を確認してから調整。" },
      { label: "元素チャージ効率", value: "190–210%" },
      { label: "会心率", value: "70%以上" },
      { label: "水元素ダメージ", value: "46.6%" },
    ],
  },
  {
    id: "bennett",
    name: "ベネット",
    element: "炎",
    role: "攻撃支援・回復",
    constellation: "無凸前提",
    imageUrl: "https://gi.yatta.moe/assets/UI/UI_AvatarIcon_Bennett.png",
    weapon: "原木刀",
    weaponImageUrl:
      "https://gi.yatta.moe/assets/UI/UI_EquipIcon_Sword_Arakalari.png",
    artifact: "旧貴族のしつけ",
    artifactImageUrl:
      "https://gi.yatta.moe/assets/UI/reliquary/UI_RelicIcon_15007_4.png",
    mainStats: "元素チャージ / HP% / 治療効果",
    subStats: "元素チャージ ＞ HP% ＞ HP",
    targetStats: [
      { label: "基礎攻撃力", value: "756", primary: true, note: "デモ値。基礎攻撃力と、効果を受けた後の総攻撃力を分けて確認。" },
      { label: "元素チャージ効率", value: "230%以上" },
      { label: "HP", value: "24,000以上" },
      { label: "治療効果", value: "35.9%" },
    ],
  },
  {
    id: "zhongli",
    name: "鍾離",
    element: "岩",
    role: "シールド・耐性低下",
    constellation: "無凸前提",
    imageUrl: "https://gi.yatta.moe/assets/UI/UI_AvatarIcon_Zhongli.png",
    weapon: "西風長槍",
    weaponImageUrl:
      "https://gi.yatta.moe/assets/UI/UI_EquipIcon_Pole_Zephyrus.png",
    artifact: "千岩牢固",
    artifactImageUrl:
      "https://gi.yatta.moe/assets/UI/reliquary/UI_RelicIcon_15017_4.png",
    mainStats: "HP% / HP% / HP%",
    subStats: "HP% ＞ HP ＞ 会心率",
    targetStats: [
      { label: "HP", value: "45,000以上", primary: true },
      { label: "会心率", value: "45%以上", note: "デモ値。武器効果の発動条件と、戦闘中の会心率加算を確認。" },
      { label: "元素チャージ効率", value: "140–160%" },
    ],
  },
];
