import * as $ from 'svelte/internal/server';
import { thing } from './mod.js';
export default function Input($$renderer) {
	// leads the import
	let y = thing;
	$$renderer.push(`<p>${$.escape(y)}</p>`);
}
