import * as $ from 'svelte/internal/server';
import { a } from './a.js';
import { b } from './b.js';
export default function Input($$renderer) {
	// between imports
	let y = a + b;
	$$renderer.push(`<p>${$.escape(y)}</p>`);
}
