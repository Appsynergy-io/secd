/** Build an element with attributes and children. `true` sets a bare attribute,
 *  `false`/undefined skips it. Text children become text nodes. */

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | boolean | undefined> = {},
  children: Array<Node | string> = [],
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === undefined || v === false) {
      continue;
    }
    node.setAttribute(k, v === true ? "" : v);
  }
  for (const child of children) {
    node.append(typeof child === "string" ? document.createTextNode(child) : child);
  }
  return node;
}

/** Type guards for nodes found by query. */
export function asButton(node: Element | null): HTMLButtonElement | null {
  return node instanceof HTMLButtonElement ? node : null;
}

export function asInput(node: Element | null): HTMLInputElement | null {
  return node instanceof HTMLInputElement ? node : null;
}
