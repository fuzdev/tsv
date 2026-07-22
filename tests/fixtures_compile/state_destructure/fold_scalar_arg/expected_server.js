import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let n = 5;
	let tmp = n,
		a = tmp.a;
	$$renderer.push(`<!---->5`);
}
