import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$$renderer.push(
		`<svg><foreignObject><thisshouldwarnme>x</thisshouldwarnme></foreignObject></svg>`
	);
}
