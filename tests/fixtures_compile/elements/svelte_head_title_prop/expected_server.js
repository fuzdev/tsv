import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { t } = $$props;
	$.head('4hbqx4', $$renderer, ($$renderer) => {
		$$renderer.title(($$renderer) => {
			$$renderer.push(`<title>page ${$.escape(t)}</title>`);
		});
	});
}
