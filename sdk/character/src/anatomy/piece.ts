// A piece of the anatomy: one bone, cartilage, organ or vessel, its mesh in its joint's frame at rest (the joint's head
// at the origin), so the joint's turn carries it as the body is posed (character/drawing.tsx onJoint).
import type { Frame } from "./frame";
import { shifted, type Geo } from "./mesh";

export type Kind = "bone" | "cartilage" | "organ" | "vessel" | "airway";

export interface Piece {
  /** what it is ("rib-5-left", "lung-right", "heart") */
  name: string;
  kind: Kind;
  /** the joint it rides */
  joint: string;
  geo: Geo;
  /** its colour as it is (sRGB hex), and how rough */
  color: string;
  roughness: number;
}

export const COLORS = {
  bone: "#e6dcc6", cartilage: "#cdd6d2", lung: "#d79a96", heart: "#8a2c2a", liver: "#6b2b20", stomach: "#c4897a",
  bowel: "#d3a089", colon: "#c49378", kidney: "#7a302b", spleen: "#5e2734", aorta: "#a8383a", airway: "#d6cdbd",
};

export function piece(F: Frame, name: string, kind: Kind, joint: string, geo: Geo, color: string, roughness = 0.6): Piece {
  return { name, kind, joint, geo: shifted(geo, F.joint(joint)), color, roughness };
}
