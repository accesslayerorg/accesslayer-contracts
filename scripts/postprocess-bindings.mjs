#!/usr/bin/env node
// Post-processes the raw output of `stellar contract bindings typescript` into the
// published `bindings/` package.
//
// Input layout (produced by scripts/generate-bindings.sh):
//   <work>/contracts.json          [{ name, crate, crateDir, interfaceSha256 }]
//   <work>/<name>/client/src/index.ts
//   <work>/<name>/spec.json
//   <work>/skipped.json            (optional) [{ crate, reason }]
//
// Output layout (all files are committed so CI can diff them):
//   bindings/src/contracts/<name>.ts   one module per contract (generated client)
//   bindings/src/errors.ts             typed error-enum registry
//   bindings/src/events.ts             typed event decoders + payload interfaces
//   bindings/src/index.ts              package barrel
//   bindings/manifest.json             contract -> interface hash / summary
//
// The output is intentionally deterministic: sorted keys, no timestamps, and a
// fixed header, so a regeneration on an unchanged interface produces a byte
// identical tree.

import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..");

function parseArgs(argv) {
  const args = { work: join(repoRoot, "target", "bindings-work"), out: join(repoRoot, "bindings") };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--work") args.work = resolve(argv[++i]);
    else if (arg === "--out") args.out = resolve(argv[++i]);
    else throw new Error(`unknown argument: ${arg}`);
  }
  return args;
}

// The interface hash identifies the contract spec the file was generated from.
// The WASM hash is deliberately not recorded: a WASM build embeds absolute
// toolchain paths, so its bytes (and hash) differ between machines and Rust
// releases even when the interface is unchanged. That would make the committed
// output look stale after every regeneration on a different runner.
const GENERATED_HEADER = (source, interfaceSha256) =>
  [
    "// AUTO-GENERATED FILE - DO NOT EDIT.",
    `// Source contract: ${source}`,
    `// Interface SHA-256: ${interfaceSha256}`,
    "// Regenerate with: ./scripts/generate-bindings.sh",
    "",
  ].join("\n");

const snakeToCamel = (value) => value.replace(/[-_]+([a-zA-Z0-9])/g, (_, c) => c.toUpperCase());
const pascalCase = (value) =>
  value
    .split(/[-_]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join("");

/** TypeScript namespace used to import a contract's generated types. */
const typeNamespace = (name) => {
  const pascal = pascalCase(name);
  return /Contract$/.test(pascal) ? pascal : `${pascal}Contract`;
};

// ---------------------------------------------------------------------------
// Rust event-definition scanning
// ---------------------------------------------------------------------------

/** Matches `pub const NAME: Symbol = symbol_short!("sym");` (also `Symbol::new`). */
const SYMBOL_CONST_RE =
  /pub const ([A-Za-z0-9_]+):\s*Symbol\s*=\s*(?:symbol_short!\(\s*"([^"]+)"\s*\)|Symbol::new\([^,]*,\s*"([^"]+)"\s*\))\s*;/g;

function collectRustFiles(dir) {
  const files = [];
  const walk = (current) => {
    for (const entry of readdirSync(current, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      const full = join(current, entry.name);
      if (entry.isDirectory()) {
        if (entry.name === "target" || entry.name === "test_snapshots") continue;
        walk(full);
      } else if (entry.isFile() && entry.name.endsWith(".rs")) {
        // Test modules publish events too; they are not part of the interface.
        if (/(^|\/)test[A-Za-z0-9_]*\.rs$/.test(entry.name) || entry.name === "tests.rs") continue;
        files.push(full);
      }
    }
  };
  walk(dir);
  return files;
}

/** Splits `a, b(c, d), e` into top-level parts. */
function splitTopLevelCommas(value) {
  const parts = [];
  let depth = 0;
  let current = "";
  let inString = false;
  for (let i = 0; i < value.length; i += 1) {
    const char = value[i];
    if (char === '"' && value[i - 1] !== "\\") inString = !inString;
    if (!inString) {
      if ("([{<".includes(char)) depth += 1;
      else if (")]}>" .includes(char)) depth -= 1;
      else if (char === "," && depth === 0) {
        parts.push(current.trim());
        current = "";
        continue;
      }
    }
    current += char;
  }
  if (current.trim() !== "") parts.push(current.trim());
  return parts;
}

/** Returns the text between the parenthesis that opens at `openIndex` and its match. */
function readParenthesised(text, openIndex) {
  let depth = 0;
  let inString = false;
  for (let i = openIndex; i < text.length; i += 1) {
    const char = text[i];
    if (char === '"' && text[i - 1] !== "\\") inString = !inString;
    if (inString) continue;
    if (char === "(") depth += 1;
    else if (char === ")") {
      depth -= 1;
      if (depth === 0) return text.slice(openIndex + 1, i);
    }
  }
  return null;
}

function resolveTopicSymbol(expr, symbols) {
  const trimmed = expr.trim();
  const short = /^[^"'`]*?symbol_short!\(\s*"([^"]+)"\s*\)/.exec(trimmed);
  if (short) return short[1];
  const symbolNew = /Symbol::new\([^,]+,\s*"([^"]+)"\s*\)/.exec(trimmed);
  if (symbolNew) return symbolNew[1];
  const fromConst = /^([A-Za-z0-9_:]+)$/.exec(trimmed);
  if (fromConst) {
    const name = fromConst[1].split("::").pop();
    if (symbols.has(name)) return symbols.get(name);
  }
  return null;
}

/** Resolves the payload expression of a publish call to a `#[contracttype]` name, when possible. */
function resolvePayloadType(payload, fileText) {
  const trimmed = payload.trim().replace(/^&\s*/, "");
  const literal = /^([A-Z][A-Za-z0-9_]*)\s*\{/.exec(trimmed);
  if (literal) return { struct: literal[1] };
  if (/^\(/.test(trimmed)) return { tuple: true };
  const identifier = /^([a-z_][A-Za-z0-9_]*)$/.exec(trimmed);
  if (identifier) {
    const name = identifier[1];
    const annotated = new RegExp(`let\\s+${name}\\s*:\\s*([A-Z][A-Za-z0-9_]*)`).exec(fileText);
    if (annotated) return { struct: annotated[1] };
    const inferred = new RegExp(`let\\s+${name}\\s*=\\s*&?\\s*([A-Z][A-Za-z0-9_]*)\\s*\\{`).exec(fileText);
    if (inferred) return { struct: inferred[1] };
    return { struct: null };
  }
  return { struct: null };
}

/**
 * Extracts documented event shapes from a contract's Rust sources.
 *
 * The repository documents event shapes on the payload struct/type:
 *
 *   /// Event shape:
 *   /// - topics: `(FEE_ADJUSTED_EVENT_NAME, key_id)`
 *   /// - data: `FeeAdjustedEvent`
 *   #[contracttype]
 *   pub struct FeeAdjustedEvent { ... }
 *
 * The first topic entry names the event-name constant, which is how we resolve
 * the on-chain topic symbol. Payload types are resolved from the contract's
 * Rust definition (fields + types) and cross-checked against the contract spec.
 */
/**
 * Extracts every event a contract publishes, with the best payload type we can
 * resolve statically.
 *
 * Two sources are combined, because this repository uses both conventions:
 * 1. `env.events().publish((TOPIC, ...), payload)` call sites, which give the
 *    on-chain topic symbol and the payload expression.
 * 2. `Event shape` doc blocks on the payload type (`- topics: \`(CONST, a)\``,
 *    `- data: \`PayloadType\``), which are the most explicit definition and win
 *    when both are present.
 */
export function extractRustEvents(crateDir) {
  const events = new Map();

  const record = (name, details) => {
    const existing = events.get(name);
    if (existing === undefined) {
      events.set(name, { name, topics: details.topics ?? null, payloadStruct: null, ...details });
      return;
    }
    events.set(name, {
      ...existing,
      ...details,
      topics: details.topics ?? existing.topics,
      payloadStruct: details.payloadStruct ?? existing.payloadStruct,
    });
  };

  for (const file of collectRustFiles(join(repoRoot, crateDir))) {
    const text = readFileSync(file, "utf8");
    const source = relative(repoRoot, file);
    const symbols = new Map();
    for (const match of text.matchAll(SYMBOL_CONST_RE)) {
      symbols.set(match[1], match[2] ?? match[3]);
    }

    // 1) publish call sites
    let index = text.indexOf(".publish(");
    while (index !== -1) {
      const inner = readParenthesised(text, index + ".publish(".length - 1);
      if (inner !== null) {
        const args = splitTopLevelCommas(inner);
        if (args.length >= 2) {
          const topicExpr = args[0];
          const topics = /^\(/.test(topicExpr)
            ? splitTopLevelCommas(topicExpr.slice(1, -1)).map((part) => part.trim())
            : [topicExpr.trim()];
          const name = resolveTopicSymbol(topics[0] ?? "", symbols);
          if (name !== null) {
            const payload = resolvePayloadType(args.slice(1).join(","), text);
            record(name, {
              topics: topics.length,
              payloadStruct: payload.struct ?? null,
              tuplePayload: payload.tuple === true,
              source,
            });
          }
        }
      }
      index = text.indexOf(".publish(", index + 1);
    }

    // 2) documented event shapes
    const lines = text.split("\n");
    let doc = [];
    for (let i = 0; i < lines.length; i += 1) {
      const line = lines[i];
      if (/^\s*\/\/\//.test(line)) {
        doc.push(line.replace(/^\s*\/\/\/\s?/, ""));
        continue;
      }
      const structMatch = /^\s*pub struct ([A-Za-z0-9_]+)/.exec(line);
      if (structMatch && doc.length > 0) {
        const shape = parseShapeDoc(doc);
        const symbol = shape ? symbols.get(shape.topicConst) : undefined;
        if (symbol) {
          record(symbol, {
            constName: shape.topicConst,
            payloadStruct: structMatch[1],
            dataType: shape.dataType ?? structMatch[1],
            fields: parseStructFields(lines, i),
            source,
          });
        }
      }
      if (line.trim() !== "" && !/^\s*#\[/.test(line)) doc = [];
    }
  }

  return [...events.values()].sort((a, b) => a.name.localeCompare(b.name));
}

function parseShapeDoc(docLines) {
  const doc = docLines.join("\n");
  const topics = /- topics:\s*`\(([^)]*)\)`/.exec(doc);
  if (!topics) return null;
  const parts = topics[1]
    .split(",")
    .map((part) => part.trim())
    .filter(Boolean);
  if (parts.length === 0) return null;
  const data = /- data:\s*`([^`]+)`/.exec(doc);
  return { topicConst: parts[0], dataType: data ? data[1].trim() : undefined };
}

function parseStructFields(lines, structLineIndex) {
  const fields = [];
  for (let i = structLineIndex; i < lines.length; i += 1) {
    const line = lines[i];
    if (/^\s*pub struct/.test(line) && i !== structLineIndex) break;
    const closing = /^\s*}\s*$/.test(line);
    if (closing) break;
    const field = /^\s*pub ([A-Za-z0-9_]+):\s*(.+?),\s*$/.exec(line);
    if (field) fields.push({ name: field[1], rustType: field[2].trim() });
  }
  return fields;
}

// ---------------------------------------------------------------------------
// Rust -> TypeScript type mapping
// ---------------------------------------------------------------------------

function rustTypeToTs(rustType, exportedTypes) {
  const type = rustType.trim();
  if (type === "()") return "void";
  if (/^(u8|u16|u32|i8|i16|i32)$/.test(type)) return "number";
  if (/^(u64|i64|u128|i128|u256|i256|Timepoint|Duration)$/.test(type)) return "bigint";
  if (type === "bool") return "boolean";
  if (/^(Address|String|Symbol|ScString|ScSymbol)$/.test(type)) return "string";
  if (/^BytesN<\s*[0-9]+\s*>$/.test(type) || type === "Bytes") return "Buffer";

  const option = /^Option<\s*(.+)\s*>$/.exec(type);
  if (option) return `${rustTypeToTs(option[1], exportedTypes)} | null`;

  const vec = /^Vec<\s*(.+)\s*>$/.exec(type);
  if (vec) return `Array<${rustTypeToTs(vec[1], exportedTypes)}>`;

  const map = /^Map<\s*(.+),\s*(.+)>$/.exec(type);
  if (map) return `Map<${rustTypeToTs(map[1], exportedTypes)}, ${rustTypeToTs(map[2], exportedTypes)}>`;

  if (/^\(.*\)$/.test(type)) {
    const inner = type.slice(1, -1).trim();
    if (inner === "") return "readonly []";
    const parts = splitTopLevel(inner);
    return `readonly [${parts.map((part) => rustTypeToTs(part, exportedTypes)).join(", ")}]`;
  }

  const named = type.replace(/^crate::/, "").split("::").pop();
  return exportedTypes.has(named) ? named : "unknown";
}

function splitTopLevel(value) {
  const parts = [];
  let depth = 0;
  let current = "";
  for (const char of value) {
    if (char === "<" || char === "(" || char === "[") depth += 1;
    if (char === ">" || char === ")" || char === "]") depth -= 1;
    if (char === "," && depth === 0) {
      parts.push(current.trim());
      current = "";
      continue;
    }
    current += char;
  }
  if (current.trim() !== "") parts.push(current.trim());
  return parts;
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

const RESERVED_WORDS = new Set([
  "await", "break", "case", "catch", "class", "const", "continue", "debugger", "default", "delete",
  "do", "else", "enum", "export", "extends", "false", "finally", "for", "function", "if", "implements",
  "import", "in", "instanceof", "interface", "let", "new", "null", "package", "private", "protected",
  "public", "return", "static", "super", "switch", "this", "throw", "true", "try", "typeof", "var",
  "void", "while", "with", "yield",
]);

/**
 * `stellar contract bindings typescript` emits parameter destructuring verbatim
 * from the contract spec, so a contract parameter named after a JavaScript
 * reserved word (for example `new` in `propose_threshold_update`) produces code
 * that does not parse.
 *
 * TypeScript does not allow renaming (`{new: new_}`) inside a parameter pattern
 * of a function *type*, so the destructuring is replaced with a plain object
 * parameter. Property names, and therefore the wire format, are unchanged.
 */
function sanitizeReservedBindings(source) {
  return source
    .split("\n")
    .map((line) => {
      if (!line.includes("({") || !line.includes("}:")) return line;
      const pattern = /\(\{([^{}]*)\}: /.exec(line);
      if (pattern === null) return line;
      const hasReservedBinding = splitTopLevelCommas(pattern[1]).some((part) => {
        const bare = /^([A-Za-z_$][A-Za-z0-9_$]*)$/.exec(part.trim());
        return bare !== null && RESERVED_WORDS.has(bare[1]);
      });
      return hasReservedBinding ? line.replace(/\(\{[^{}]*\}: /, "(args: ") : line;
    })
    .join("\n");
}

function listExportedTypes(clientSource) {
  const names = new Set();
  for (const match of clientSource.matchAll(/^export (?:interface|type|const|enum) ([A-Za-z0-9_]+)/gm)) {
    names.add(match[1]);
  }
  return names;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const contractsFile = join(args.work, "contracts.json");
  if (!existsSync(contractsFile)) {
    throw new Error(`missing ${contractsFile}; run scripts/generate-bindings.sh`);
  }
  const descriptors = JSON.parse(readFileSync(contractsFile, "utf8")).sort((a, b) => a.name.localeCompare(b.name));
  const skippedFile = join(args.work, "skipped.json");
  const skipped = existsSync(skippedFile) ? JSON.parse(readFileSync(skippedFile, "utf8")) : [];
  const packageJsonPath = join(args.out, "package.json");
  if (!existsSync(packageJsonPath)) throw new Error(`missing ${packageJsonPath}`);
  const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));

  const srcDir = join(args.out, "src");
  const contractsOut = join(srcDir, "contracts");
  rmSync(contractsOut, { recursive: true, force: true });
  mkdirSync(contractsOut, { recursive: true });

  const manifestContracts = [];
  const indexLines = ["// AUTO-GENERATED FILE - DO NOT EDIT.", "// Regenerate with: ./scripts/generate-bindings.sh", ""];
  const errorImports = [];
  const errorRegistry = [];
  const eventModules = [];
  const eventImportLines = [];

  for (const descriptor of descriptors) {
    const clientPath = join(args.work, descriptor.name, "client", "src", "index.ts");
    const specPath = join(args.work, descriptor.name, "spec.json");
    const client = sanitizeReservedBindings(readFileSync(clientPath, "utf8")).replace(/\s+$/, "\n");
    const spec = JSON.parse(readFileSync(specPath, "utf8"));
    const alias = snakeToCamel(descriptor.name);

    writeFileSync(
      join(contractsOut, `${descriptor.name}.ts`),
      `${GENERATED_HEADER(descriptor.crateDir, descriptor.interfaceSha256)}${client}`,
    );
    indexLines.push(`export * as ${alias} from "./contracts/${descriptor.name}.js";`);

    // --- error enums -------------------------------------------------------
    const errorEnums = spec
      .filter((entry) => entry.udt_error_enum_v0)
      .map((entry) => entry.udt_error_enum_v0)
      .sort((a, b) => a.name.localeCompare(b.name));
    const exportedTypes = listExportedTypes(client);
    const registryEntries = [];
    for (const errorEnum of errorEnums) {
      if (!exportedTypes.has(errorEnum.name)) continue;
      const local = `${alias}_${errorEnum.name}`;
      errorImports.push(`import { ${errorEnum.name} as ${local} } from "./contracts/${descriptor.name}.js";`);
      registryEntries.push(`    "${errorEnum.name}": ${local},`);
    }
    if (registryEntries.length > 0) {
      errorRegistry.push(`  "${descriptor.name}": {\n${registryEntries.join("\n")}\n  },`);
    }

    // --- events ------------------------------------------------------------
    const events = extractRustEvents(descriptor.crateDir);
    if (events.length > 0) {
      const namespace = typeNamespace(descriptor.name);
      eventImportLines.push(`import type * as ${namespace} from "./contracts/${descriptor.name}.js";`);
      const payloadEntries = events.map((event) => {
        const payloadType = payloadTs(event, exportedTypes, namespace);
        return `  ${JSON.stringify(event.name)}: ${payloadType};`;
      });
      const nameList = events.map((event) => JSON.stringify(event.name)).join(", ");
      const decodedType = `${pascalCase(descriptor.name)}DecodedEvent`;
      const payloadTypeName = `${pascalCase(descriptor.name)}EventPayloads`;
      eventModules.push(
        [
          `/** Payload types for every documented \`${descriptor.name}\` event. */`,
          `export interface ${payloadTypeName} {`,
          payloadEntries.join("\n"),
          `}`,
          "",
          `export type ${pascalCase(descriptor.name)}EventName = keyof ${payloadTypeName};`,
          "",
          `export interface ${decodedType}<N extends ${pascalCase(descriptor.name)}EventName = ${pascalCase(descriptor.name)}EventName> {`,
          `  name: N;`,
          `  contract: ${JSON.stringify(descriptor.name)};`,
          `  topics: readonly unknown[];`,
          `  data: ${payloadTypeName}[N];`,
          `}`,
          "",
          `const ${alias}EventNames: ReadonlySet<string> = new Set([${nameList}]);`,
          "",
          `/** Decodes a \`${descriptor.name}\` contract event, or returns \`null\` for events it does not know. */`,
          `export function decode${pascalCase(descriptor.name)}Event(`,
          `  event: RawContractEvent,`,
          `): ${decodedType} | null {`,
          `  const decoded = decodeEvent(event);`,
          `  if (decoded === null || !${alias}EventNames.has(decoded.name)) return null;`,
          `  return decoded as unknown as ${decodedType};`,
          `}`,
          "",
        ].join("\n"),
      );
    }

    manifestContracts.push({
      name: descriptor.name,
      crate: descriptor.crate,
      crateDir: descriptor.crateDir,
      interfaceSha256: descriptor.interfaceSha256,
      entrypoints: spec.filter((entry) => entry.function_v0).length,
      errorEnums: errorEnums.map((entry) => entry.name),
      events: events.map((event) => event.name),
    });
  }

  // errors.ts ---------------------------------------------------------------
  writeFileSync(
    join(srcDir, "errors.ts"),
    [
      "// AUTO-GENERATED FILE - DO NOT EDIT.",
      "// Typed error-enum registry. Every `#[contracterror]` enum from every",
      "// generated contract spec is re-exported under its contract name, so callers",
      "// can map a raw `u32` error code to the variant name and message.",
      '// Regenerate with: ./scripts/generate-bindings.sh',
      "",
      ...dedupe(errorImports),
      "",
      "export const contractErrorEnums = {",
      ...errorRegistry,
      "} as const;",
      "",
      "export type ContractName = keyof typeof contractErrorEnums;",
      "",
      "/**",
      " * Resolves a raw contract error code (the `u32` inside a failed `Result`) to",
      " * the typed enum variant exported by that contract's client.",
      " */",
      "export function describeContractError(",
      "  contract: ContractName,",
      "  enumName: string,",
      "  code: number,",
      "): { name: string; message: string } | undefined {",
      "  const enums = contractErrorEnums[contract] as Record<string, Record<number, { message: string }>>;",
      "  const errorEnum = enums[enumName];",
      "  if (errorEnum === undefined) return undefined;",
      "  const variant = errorEnum[code];",
      "  if (variant === undefined) return undefined;",
      "  return { name: enumName, message: variant.message };",
      "}",
      "",
    ].join("\n"),
  );

  // events.ts ---------------------------------------------------------------
  writeFileSync(
    join(srcDir, "events.ts"),
    [
      "// AUTO-GENERATED FILE - DO NOT EDIT.",
      "// Typed event decoders derived from the contracts' event definitions",
      "// (the `Event shape` doc blocks on each payload type in the Rust sources)",
      "// and the payload types in the contract spec.",
      '// Regenerate with: ./scripts/generate-bindings.sh',
      "",
      'import type { RawContractEvent } from "./decode.js";',
      'import { decodeEvent } from "./decode.js";',
      ...dedupe(eventImportLines),
      "",
      ...eventModules,
    ].join("\n"),
  );

  // index.ts ----------------------------------------------------------------
  indexLines.push(
    "",
    'export { decodeEvent, type RawContractEvent, type DecodedContractEvent } from "./decode.js";',
    'export * from "./errors.js";',
    'export * from "./events.js";',
    "",
  );
  writeFileSync(join(srcDir, "index.ts"), indexLines.join("\n"));

  // manifest.json -----------------------------------------------------------
  const manifest = {
    package: packageJson.name,
    version: packageJson.version,
    sorobanSdk: readSdkVersion(),
    generator: "scripts/generate-bindings.sh",
    contracts: manifestContracts,
    skipped,
  };
  writeFileSync(join(args.out, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);

  const eventCount = manifestContracts.reduce((total, contract) => total + contract.events.length, 0);
  console.log(
    `Generated bindings for ${manifestContracts.length} contracts ` +
      `(${manifestContracts.reduce((total, contract) => total + contract.entrypoints, 0)} entrypoints, ` +
      `${eventCount} event decoders).`,
  );
  if (skipped.length > 0) {
    console.log(`Skipped ${skipped.length} contract(s): ${skipped.map((entry) => entry.crate).join(", ")}`);
  }
}

function payloadTs(event, exportedTypes, namespace) {
  if (event.dataType && event.dataType !== event.payloadStruct) {
    const mapped = rustTypeToTs(event.dataType, exportedTypes);
    return mapped.replace(/(^|[\s<[,|])([A-Z][A-Za-z0-9_]*)/g, (match, prefix, name) =>
      exportedTypes.has(name) ? `${prefix}${namespace}.${name}` : match,
    );
  }
  if (event.payloadStruct && exportedTypes.has(event.payloadStruct)) {
    return `${namespace}.${event.payloadStruct}`;
  }
  if (event.tuplePayload && !event.payloadStruct) return "readonly unknown[]";
  return "unknown";
}

function dedupe(values) {
  return [...new Set(values)];
}

function readSdkVersion() {
  const cargoToml = join(repoRoot, "Cargo.toml");
  if (!existsSync(cargoToml)) return "unknown";
  const match = /soroban-sdk\s*=\s*"([^"]+)"/.exec(readFileSync(cargoToml, "utf8"));
  return match ? match[1] : "unknown";
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main();
}
