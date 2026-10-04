/**
 * Trusted Core が提供する catalog-v2 の Renderer 向け型。
 *
 * カタログはビルド時に検証されるため、この層ではゲームデータの
 * 解釈や補正を行わず、受け取った値をそのまま表現する。
 */
export interface Character {
  id: string;
  name: string;
  element: string;
  weaponType: string;
  rarity: number;
  imageUrl: string;
}

export interface Weapon {
  id: string;
  name: string;
  weaponType: string;
  rarity: number;
  imageUrl: string;
}

export interface PieceImageUrls {
  flower: string;
  plume: string;
  sands: string;
  goblet: string;
  circlet: string;
}

export interface ArtifactSet {
  id: string;
  name: string;
  teamBuffKey: string | null;
  twoPieceEffectGroupId: string;
  twoPieceEffect: string;
  fourPieceEffect: string | null;
  pieceImageUrls: PieceImageUrls;
}

export interface Catalog {
  schemaVersion: "catalog-v2";
  gameVersion: string;
  catalogUpdatedAt: string;
  characters: ReadonlyArray<Character>;
  weapons: ReadonlyArray<Weapon>;
  artifactSets: ReadonlyArray<ArtifactSet>;
}

export interface StarRailCatalog {
  schemaVersion: "star-rail-catalog-v1";
  game: "star_rail";
  gameVersion: string;
  catalogUpdatedAt: string;
  characters: { id: string; name: string; element: string; path: string; aliases: string[]; exclusiveGroup?: string | null; imageUrl: string | null }[];
  lightCones: { id: string; name: string; path: string; imageUrl: string | null; legacyOnly?: boolean }[];
  tunnelRelics: { id: string; name: string; imageUrl: string | null; legacyOnly?: boolean }[];
  ornaments: { id: string; name: string; imageUrl: string | null; legacyOnly?: boolean }[];
}
