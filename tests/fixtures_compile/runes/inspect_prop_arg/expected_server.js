import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { $$slots, $$events, ...props } = $$props;
		$$renderer.push(`<!---->${$.escape(props.foo)}`);
	});
}
