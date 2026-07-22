import * as $ from 'svelte/internal/server';
import { i } from './i.js';
import { m } from './m.js';
export default function Input($$renderer) {
	let a = 1;
	$$renderer.push(`<p>1${$.escape(i)}${$.escape(m)}</p>`);
}
