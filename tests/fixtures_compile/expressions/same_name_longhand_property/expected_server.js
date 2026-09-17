import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { active = false, label } = $$props;
		let count = 0;
		let doubled = $.derived(() => count * 2);
		const options = { active, doubled: doubled(), label };
		function read(source) {
			const { count = 1 } = source;
			return { count };
		}
		function describe(o) {
			return `${o.label}`;
		}
		$$renderer.push(
			`<p${$.attr('title', describe({ label }))}>${$.escape(options.doubled)}${$.escape(read({}).count)}</p>`
		);
	});
}
