import * as $ from 'svelte/internal/server';
import { bar } from './bar.ts';
export default function Input($$renderer) {
	let x = bar;
	$$renderer.push(`<p>${$.escape(x)}</p>`);
}
