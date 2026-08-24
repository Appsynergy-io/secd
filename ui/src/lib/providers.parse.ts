/** Parse locked providers from secd-core/src/provider.rs. */

export type ProviderField = {
  key: string;
  secret: boolean;
  optional: boolean;
  env: string;
};

export type Provider = {
  name: string;
  title: string;
  fields: ProviderField[];
};

const CALL =
  /\b(req|req_secret|opt|opt_secret)\(\s*"([^"]+)"\s*,\s*"([^"]+)"\s*\)/g;

const KIND = {
  req: { secret: false, optional: false },
  req_secret: { secret: true, optional: false },
  opt: { secret: false, optional: true },
  opt_secret: { secret: true, optional: true },
} as const;

function fnBody(src: string, name: string): string {
  const start = src.indexOf(`fn ${name}(`);
  if (start < 0) {
    throw new Error(`providers.gen: missing fn ${name}`);
  }
  const brace = src.indexOf("{", start);
  if (brace < 0) {
    throw new Error(`providers.gen: missing body for ${name}`);
  }
  let depth = 0;
  for (let i = brace; i < src.length; i++) {
    const c = src[i];
    if (c === "{") {
      depth += 1;
    } else if (c === "}") {
      depth -= 1;
      if (depth === 0) {
        return src.slice(brace + 1, i);
      }
    }
  }
  throw new Error(`providers.gen: unclosed fn ${name}`);
}

function expandShareLoops(src: string): string {
  const header = /for i in 1..=(\d+)\s*\{/g;
  let out = "";
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = header.exec(src)) !== null) {
    const n = Number(m[1]);
    const bodyStart = header.lastIndex;
    let depth = 1;
    let end = -1;
    for (let i = bodyStart; i < src.length; i++) {
      const c = src[i];
      if (c === "{") {
        depth += 1;
      } else if (c === "}") {
        depth -= 1;
        if (depth === 0) {
          end = i;
          break;
        }
      }
    }
    if (end < 0) {
      throw new Error("providers.gen: unclosed share loop");
    }
    const body = src.slice(bodyStart, end);
    const formats = [...body.matchAll(/format!\("([^"]+)"\)/g)].map((x) => x[1]);
    if (formats.length < 2 || formats[0] === undefined || formats[1] === undefined) {
      throw new Error("providers.gen: share loop needs two format! strings");
    }
    let macro: keyof typeof KIND = "opt_secret";
    if (body.includes("req_secret")) {
      macro = "req_secret";
    } else if (/\breq\(/.test(body)) {
      macro = "req";
    } else if (/\bopt\(/.test(body)) {
      macro = "opt";
    }
    const lines: string[] = [];
    for (let i = 1; i <= n; i++) {
      const key = formats[0].replaceAll("{i}", String(i));
      const env = formats[1].replaceAll("{i}", String(i));
      lines.push(`${macro}("${key}", "${env}")`);
    }
    out += src.slice(last, m.index) + lines.join("\n");
    last = end + 1;
  }
  return out + src.slice(last);
}

function fieldsFromCalls(src: string): ProviderField[] {
  const out: ProviderField[] = [];
  for (const m of src.matchAll(CALL)) {
    const macro = m[1];
    const key = m[2];
    const env = m[3];
    if (
      macro !== "req" &&
      macro !== "req_secret" &&
      macro !== "opt" &&
      macro !== "opt_secret"
    ) {
      continue;
    }
    if (key === undefined || env === undefined) {
      continue;
    }
    out.push({ key, env, ...KIND[macro] });
  }
  return out;
}

export function parseProviderRs(src: string): Provider[] {
  const vault = fieldsFromCalls(expandShareLoops(fnBody(src, "vault_fields")));
  const body = fnBody(src, "builtins");
  const out: Provider[] = [];
  const re =
    /Provider \{\s*name: "([^"]+)"\.to_string\(\),\s*title: "([^"]+)"\.to_string\(\),\s*fields:\s*/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(body)) !== null) {
    const name = m[1];
    const title = m[2];
    if (name === undefined || title === undefined) {
      throw new Error("providers.gen: provider missing name");
    }
    const rest = body.slice(re.lastIndex);
    let fields: ProviderField[];
    if (rest.startsWith("vault_fields()")) {
      fields = vault;
    } else if (rest.startsWith("vec![")) {
      const start = 4;
      let depth = 0;
      let end = -1;
      for (let i = start; i < rest.length; i++) {
        const c = rest[i];
        if (c === "[") {
          depth += 1;
        } else if (c === "]") {
          depth -= 1;
          if (depth === 0) {
            end = i;
            break;
          }
        }
      }
      if (end < 0) {
        throw new Error(`providers.gen: unclosed fields for ${name}`);
      }
      fields = fieldsFromCalls(rest.slice(start, end));
    } else {
      throw new Error(`providers.gen: unknown fields for ${name}`);
    }
    out.push({ name, title, fields });
  }
  if (out.length === 0) {
    throw new Error("providers.gen: no providers parsed");
  }
  return out;
}

export function providersTs(providers: Provider[]): string {
  const field = (f: ProviderField): string =>
    `      { key: ${JSON.stringify(f.key)}, secret: ${f.secret}, optional: ${f.optional}, env: ${JSON.stringify(f.env)} }`;
  const block = (p: Provider): string => {
    const fields = p.fields.map(field).join(",\n");
    return `  {
    name: ${JSON.stringify(p.name)},
    title: ${JSON.stringify(p.title)},
    fields: [
${fields},
    ],
  }`;
  };
  return `/** Generated from crates/secd-core/src/provider.rs. Do not edit. */

export type ProviderField = {
  key: string;
  secret: boolean;
  optional: boolean;
  env: string;
};

export type Provider = {
  name: string;
  title: string;
  fields: readonly ProviderField[];
};

export const PROVIDERS: readonly Provider[] = [
${providers.map(block).join(",\n")},
];
`;
}
