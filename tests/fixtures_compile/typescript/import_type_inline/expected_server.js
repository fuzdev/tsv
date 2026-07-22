import * as $ from 'svelte/internal/server';
import { bar } from './m.ts';
export default function Input($$renderer) {
	let x = bar;
	$$renderer.push(`<p>${$.escape(x)}</p>`);
}
