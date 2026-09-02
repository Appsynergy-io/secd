/** A counter bumped on every sign-out. A screen captures it before an await and
 *  drops the response when it moved, so nothing paints into a signed-out tab. */

let logoutGen = 0;

export function currentLogoutGen(): number {
  return logoutGen;
}

export function bumpLogoutGen(): void {
  logoutGen += 1;
}
