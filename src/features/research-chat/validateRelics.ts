import type { ResearchMemberInput } from "./types";
import type { StarRailCatalog } from "../../domain/catalogTypes";

export function validateRelicInput(member: ResearchMemberInput, catalog: StarRailCatalog | null): string | null {
  if (!catalog) return "カタログを読み込んでから調査してください。指定は保持しています。";
  const character = catalog.characters.find(entry => entry.name === member.name);
  if (!character) return `${member.name}の形態・運命またはカタログ登録を確認してください。`;
  if (member.weapon && !catalog.lightCones.some(cone => cone.name === member.weapon && cone.path === character.path)) return `光円錐「${member.weapon}」は未登録または運命が一致しません。再選択してください。`;
  const tunnel = member.relics?.tunnel;
  const names = tunnel?.kind === "four_piece" ? [tunnel.set] : tunnel?.sets ?? [];
  if (names.some(name => !name)) return "2＋2の両セットまたは4セットの名称を選択してください。";
  if (names.length === 2 && names[0] === names[1]) return "2＋2には異なるセットを選択してください。";
  if (names.some(name => !catalog.tunnelRelics.some(entry => entry.name === name))) return "トンネル遺物の指定は未登録またはカテゴリが異なります。再選択してください。";
  if (member.relics?.ornament && !catalog.ornaments.some(entry => entry.name === member.relics?.ornament)) return "オーナメントの指定は未登録またはカテゴリが異なります。再選択してください。";
  return null;
}

