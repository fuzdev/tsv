<script lang="ts">
	// Deeply nested template: a chain / ternary interpolation at a deep visual indent
	// breaks in tsv (respecting printWidth at the nested position) but stays inline in
	// Prettier (its alignment reset makes the same content measure as fitting).
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
													e: '${ssss
														.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa()
														.bbbb()
														.ccccccccccc()}',
													// At 107 chars - exceeds, breaks
													e: '${ssss
														.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa()
														.bbbb()
														.ccccccccccc()}',
													// Very long chain - breaks with continuation
													f: '${ssss
														.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa()
														.bbbb()
														.ccccccccccc()}',
													// Ternary at 100 chars - stays inline
													g1: '${ssss.flagAAAAAAAA ? 'valXXXXXXXXXXXXXXXX' : 'valYYYYYYYYYYYYYYY'}',
													// Ternary at 101 chars - exceeds, breaks
													g2: '${
														ssss.flagAAAAAAAAA ? 'valXXXXXXXXXXXXXXXX' : 'valYYYYYYYYYYYYYYY'
													}',
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
													g7: '${
														ssss.flagAAAAAAAAAAAAAAAA ? 'valXXXXXXXXXXXXXXXX' : 'valYYYYYYYYYYYYYYY'
													}',
													// Inner ternary exceeds more (109 visual) - wraps and breaks
													g8: '${
														ssss.flagAAAAAAAAAAAAAAAAA
															? 'valXXXXXXXXXXXXXXXX'
															: 'valYYYYYYYYYYYYYYY'
													}',
												}`;
										})
										.join(',\n')}];
 `;
	};
</script>
