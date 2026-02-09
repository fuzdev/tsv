/**
 * Patches wasm-pack's generated package.json for npm publishing.
 * - Renames from @fuzdev/tsv_wasm to @fuzdev/tsv
 */

const pkg_path = 'crates/tsv_wasm/pkg/npm/package.json';
const pkg = JSON.parse(Deno.readTextFileSync(pkg_path));
pkg.name = '@fuzdev/tsv';
Deno.writeTextFileSync(pkg_path, JSON.stringify(pkg, null, '\t') + '\n');
console.log(`Patched ${pkg_path}: name → ${pkg.name}, version ${pkg.version}`);
