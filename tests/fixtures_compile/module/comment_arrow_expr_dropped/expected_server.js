import * as $ from 'svelte/internal/server';
const f = () => 1;
export const value = f();
export default function Input($$renderer) {
	$$renderer.push(`<p>hi</p>`);
}
