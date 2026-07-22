import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	function sum({ x, y = 2 }, [head], ...rest) {
		return x + y + head + rest.length;
	}
	let total = sum({ x: 1 }, [3], 4, 5);
	$$renderer.push(`<p>${$.escape(total)}</p>`);
}
