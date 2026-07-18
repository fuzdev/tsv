import * as $ from 'svelte/internal/server';
const K = 5;
export const v = K;
export default function Input($$renderer) {
	$$renderer.push(`<p>5</p>`);
}
