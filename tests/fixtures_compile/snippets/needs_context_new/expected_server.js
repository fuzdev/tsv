import * as $ from 'svelte/internal/server';
function stamp($$renderer) {
	$$renderer.push(`<time>${$.escape(new Date().toISOString())}</time>`);
}
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		stamp($$renderer);
	});
}
