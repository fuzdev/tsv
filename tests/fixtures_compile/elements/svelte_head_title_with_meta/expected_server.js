import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$.head('4hbqx4', $$renderer, ($$renderer) => {
		$$renderer.title(($$renderer) => {
			$$renderer.push(`<title>Hi</title>`);
		});
		$$renderer.push(`<meta name="x" content="y"/>`);
	});
}
