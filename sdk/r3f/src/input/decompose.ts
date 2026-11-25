// A matrix as position, rotation and scale, as three's Matrix4.decompose gives them, and also for a matrix with no
// volume (an axis scaled to 0: scale={0}, a pop-in's first frames, an instance hidden by scaling it away). three gives
// such a matrix the scale 1 and no rotation (its decompose, getWorldScale and extractBasis stop at a determinant of 0)
// but draws the object by the matrix itself: as nothing, or flattened. Read through three's decompose, a scale of 0
// came out as 1 and raea drew the object at full size (#164). Here it is the axes' own lengths, turned as the axes it
// keeps are.
import * as THREE from "three";

const axes = [new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3()];
const turn = new THREE.Matrix4();
const across = new THREE.Vector3();

/** `m`'s position, rotation and scale into p, q and s: three's own decompose where its determinant isn't 0 (the same
 *  numbers, bit for bit), else each axis's length as its scale (0 for an axis it flattens) and the rotation of the axes
 *  it keeps, completed to a turn. Its determinant() is three's determinantAffine() on an object's matrix, the test
 *  three's decompose makes. */
export function decompose(m: THREE.Matrix4, p: THREE.Vector3, q: THREE.Quaternion, s: THREE.Vector3): void {
  if (m.determinant() !== 0) {
    m.decompose(p, q, s);
    return;
  }
  p.setFromMatrixPosition(m);
  // the columns as they are (three's extractBasis gives such a matrix the unit axes too)
  axes.forEach((a, i) => a.setFromMatrixColumn(m, i));
  // each axis made square to the ones kept before it; one with no length, or none of its own, is scaled to 0
  const kept: number[] = [];
  axes.forEach((a, i) => {
    const length = a.length();
    for (const k of kept) a.addScaledVector(axes[k], -a.dot(axes[k]));
    const own = a.length() > 1e-9 * length && length > 0;
    s.setComponent(i, own ? length : 0);
    if (own) {
      a.normalize();
      kept.push(i);
    }
  });
  if (!kept.length) {
    q.identity();
    return;
  }
  if (kept.length === 1) {
    // a line: any axis square to it as the next (the world axis least along it, made square)
    const i = kept[0], a = axes[i], j = (i + 1) % 3;
    const along = [Math.abs(a.x), Math.abs(a.y), Math.abs(a.z)];
    across.set(0, 0, 0).setComponent(along.indexOf(Math.min(...along)), 1);
    axes[j].copy(across).addScaledVector(a, -across.dot(a)).normalize();
    kept.push(j);
  }
  if (kept.length === 2) {
    // a plane: the axis it flattens is the other two's cross product, in turn (x = y × z, y = z × x, z = x × y)
    const l = 3 - kept[0] - kept[1];
    axes[l].crossVectors(axes[(l + 1) % 3], axes[(l + 2) % 3]);
  } else if (across.crossVectors(axes[0], axes[1]).dot(axes[2]) < 0) {
    // all three kept (a determinant under the smallest number, every axis scaled tiny): a mirror on x, as three's
    axes[0].negate();
    s.x = -s.x;
  }
  q.setFromRotationMatrix(turn.makeBasis(axes[0], axes[1], axes[2]));
}
