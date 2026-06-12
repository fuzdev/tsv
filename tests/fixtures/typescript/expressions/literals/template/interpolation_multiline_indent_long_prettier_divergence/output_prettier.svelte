<script lang="ts">
	// Case 1: 2 tabs template indent
	const gen1 = () => {
		return `
		export const X = ffffff([${aaaaaaaa.bbbbbbb_cccccccc_sssss
			.map((ssss) => `'${ssss.tttttt}'`)
			.join(',')}]);
	`;
	};

	// Case 2: 3 tabs template indent
	const gen2 = () => {
		return `
			export const Y = ffffff([${aaaaaaaa.bbbbbbb_cccccccc_sssss
				.map((ssss) => `'${ssss.tttttt}'`)
				.join(',')}]);
		`;
	};

	// Case 3: 0 indent after newline
	const gen3 = () => {
		return `
${aaaaaaaa.bbbbbbb_cccccccc_ssssssssssssssssssssssssss
	.map((ssss) => `'${ssss.tttttt}'`)
	.join(',')}`;
	};

	// Case 4: No newline before ${, content line at 100 chars (fits)
	const gen4 = `${aaaaaaaa.bbbbbbb_cccccccc_ssssssssssssssssssssssssss
		.map((ssss) => `'${ssss.tttttt}'`)
		.join(',')}`;

	// Case 5: No newline before ${, content line at 101 chars (wraps)
	const gen5 = `${aaaaaaaa.bbbbbbb_cccccccc_sssssssssssssssssssssssssss
		.map((ssss) => `'${ssss.tttttt}'`)
		.join(',')}`;

	// Case 6: 4 spaces template indent → base 2, content 3 tabs, closing 2 tabs
	const gen6 = () => {
		return `
    export const A = ffffff([${aaaaaaaa.bbbbbbb_cccccccc_sssss
			.map((ssss) => `'${ssss.tttttt}'`)
			.join(',')}]);
	`;
	};

	// Case 7: 6 spaces template indent → base 3, content 4 tabs, closing 3 tabs
	const gen7 = () => {
		return `
      export const B = ffffff([${aaaaaaaa.bbbbbbb_cccccccc_sssss
				.map((ssss) => `'${ssss.tttttt}'`)
				.join(',')}]);
	`;
	};

	// Case 8: 3 spaces template indent → base 2 tabs, content 3 tabs, closing 2 tabs
	const gen8 = () => {
		return `
   export const C = ffffff([${aaaaaaaa.bbbbbbb_cccccccc_sssss
			.map((ssss) => `'${ssss.tttttt}'`)
			.join(',')}]);
	`;
	};

	// Case 9: 1 tab + 2 spaces template indent → base 2, content 3 tabs, closing 2 tabs
	const gen9 = () => {
		return `
	  export const D = ffffff([${aaaaaaaa.bbbbbbb_cccccccc_sssss
			.map((ssss) => `'${ssss.tttttt}'`)
			.join(',')}]);
	`;
	};

	// Case 10: 4 tabs template indent (deep nesting)
	const gen10 = () => {
		return `
				export const E = ffffff([${aaaaaaaa.bbbbbbb_cccccccc_sssss
					.map((ssss) => `'${ssss.tttttt}'`)
					.join(',')}]);
	`;
	};

	// Case 11: 1 tab template indent (same as script base indent)
	const gen11 = `
	export const F = ffffff([${aaaaaaaa.bbbbbbb_cccccccc_sssss
		.map((ssss) => `'${ssss.tttttt}'`)
		.join(',')}]);
`;

	// Case 12: 5 spaces template indent → base 3 tabs, content 4 tabs, closing 3 tabs
	const gen12 = () => {
		return `
     content prefix [${aaaaaaaa.bbbbbbb_cccccccc_sssss.map((ssss) => `'${ssss.tttttt}'`).join(',')}]
	`;
	};

	// Case 13: 2 tabs + 5 spaces template indent → base 5 tabs, content 6 tabs, closing 5 tabs
	const gen13 = () => {
		return `
		     deeply indented content [${aaaaaaaa.bbbbbbb_cccccccc_sssss
						.map((ssss) => `'${ssss.tttttt}'`)
						.join(',')}]
	`;
	};

	// Case 14: 4 spaces + 2 tabs template indent → base 4, content 5 tabs, closing 4 tabs
	const gen14 = () => {
		return `
    		deeply indented content [${aaaaaaaa.bbbbbbb_cccccccc_ssssssssssssssssss
					.map((ssss) => `'${ssss.tttttt}'`)
					.join(',')}]
    `;
	};

	// Case 15: 4 spaces + 1 tab + 2 spaces template indent, chain breaks use calculated tabs
	const gen15 = () => {
		return `
    	  deeply indented content [${aaaaaaaa.bbbbbbb_cccccccc_sssssssssssssssssssssss
					.map((ssss) => `'${ssss.tttttt}'`)
					.join(',')}]
    `;
	};

	// Case 16: Long inner map with block body arrow function and nested template literal
	// Tests that nested templates with their own indentation and breaking chains are handled correctly
	const gen16 = () => {
		return `
									export const Items = [${aaaaaaaa.bbbbbbb_cccccccc_sssss
										.map((ssss) => {
											const name = ssss.tttttt();
											return `{
													// Short, no newline
													a: '${ssss.a}',
													// Short - stays inline
													b: '${ssss.b}',
													// At 100 chars - stays inline
													d: '${ssss.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa().bbbb().ccccccccccc()}',
													// At 101 chars - exceeds, breaks
													e: '${ssss.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa().bbbb().ccccccccccc()}',
													// At 107 chars - exceeds, breaks
													e: '${ssss.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa().bbbb().ccccccccccc()}',
													// Very long chain - breaks with continuation
													f: '${ssss.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa().bbbb().ccccccccccc()}',
													// Ternary at 100 chars - stays inline
													g1: '${ssss.flagAAAAAAAA ? 'valXXXXXXXXXXXXXXXX' : 'valYYYYYYYYYYYYYYY'}',
													// Ternary at 101 chars - exceeds, breaks
													g2: '${ssss.flagAAAAAAAAA ? 'valXXXXXXXXXXXXXXXX' : 'valYYYYYYYYYYYYYYY'}',
													// Ternary at 100 chars - stays inline
													g3: '${ssss.flagAAAAAAAA ? 'valXXXXXXXXXXXXXXXX' : 'valYYYYYYYYYYYYYYY'}',
													// Short with line comment - forces break
													g4: '${
														ssss.short // line comment
													}',
													// Ternary with line comment - forces break
													g5: '${
														ssss.flag ? 'yes' : 'no' // line comment
													}',
													// Block comment inline - no forced break
													g6: '${ssss.val /* block comment */}',
													// Inner ternary exceeds print_width (108 visual) - wraps
													g7: '${ssss.flagAAAAAAAAAAAAAAAA ? 'valXXXXXXXXXXXXXXXX' : 'valYYYYYYYYYYYYYYY'}',
													// Inner ternary exceeds more (109 visual) - wraps and breaks
													g8: '${ssss.flagAAAAAAAAAAAAAAAAA ? 'valXXXXXXXXXXXXXXXX' : 'valYYYYYYYYYYYYYYY'}',
												}`;
										})
										.join(',\n')}];
 `;
	};
</script>
