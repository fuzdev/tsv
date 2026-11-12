/**
 * What the WASM probes' roots share, not a root itself: loading a wasm-pack `nodejs` glue by
 * path, relative to the checkout or absolute, with `node:module`'s `require`, a named
 * exception whose reason is that the `nodejs` target is CommonJS that instantiates its module
 * as it's evaluated, which no host module loads. Each load evaluates the glue afresh, so
 * each is a new instance with its own linear memory, and the memory is captured as the glue
 * constructs its instance, since the glue doesn't export it.
 *
 * @module
 */

import { createRequire } from 'node:module';
import { resolve } from 'node:path';

import { CHECKOUT_PATH } from '../../../scripts/roots/checkout.ts';
import type { FreshGlue } from '../diagnostics/wasm_memory_probe.ts';

const require = createRequire(import.meta.url);

/** Evaluates the glue at `path` afresh, returning its exports and its instance's memory. */
export const load_fresh_glue = async (path: string): Promise<FreshGlue> => {
	const resolved = require.resolve(resolve(CHECKOUT_PATH, path));
	delete require.cache[resolved];
	const Instance = WebAssembly.Instance;
	let memory: WebAssembly.Memory | null = null;
	WebAssembly.Instance = new Proxy(Instance, {
		construct(target, args, new_target) {
			const instance = Reflect.construct(target, args, new_target) as WebAssembly.Instance;
			const exported = instance.exports.memory;
			if (exported instanceof WebAssembly.Memory) memory = exported;
			return instance;
		}
	});
	let exports: FreshGlue['exports'];
	try {
		exports = require(resolved);
	} finally {
		WebAssembly.Instance = Instance;
	}
	return { exports, memory };
};
