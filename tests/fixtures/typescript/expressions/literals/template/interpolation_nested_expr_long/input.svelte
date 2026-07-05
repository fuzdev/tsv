<script lang="ts">
	const fn = () => {
		return `
									export const Items = [${aaaaaaaa.bbbbbbb_cccccccc_sssss
										.map((ssss) => {
											const name = ssss.tttttt();
											return `{
													a: '${
														ssss.flagAAAAAAAAAAAAAAAAA
															? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
																	.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
																	.ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc
																	.ddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd
															: `${
																	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
																}`
													}',
													b: '${
														ssss.flagBBBBBBBBBBBBBBBBBBB
															? `${
																	bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
																}`
															: ssss.data
																	.fallbackBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
													}',
													// inner template is 101 - wraps
													c: '${
														ssss.conditionCCCCCCCCCCCCCCCC &&
														`${
															cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc
														}`
													}',
													// inner template is 100 - does not wrap
													d: '${
														ssss.fallbackDDDDDDDDDDDDDDDDD ||
														`${ddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd}`
													}',
													f: '${ssss.processFFFFFFFFFFFFFFFFF(
														`${fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff}`
													)}',
													g: '${ssss.itemsGGGGGGGGGGGGGGGGGG.map(
														(x) =>
															`${ggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg}`
													)}',
												}`;
										})
										.join(',\n')}];
 `;
	};
</script>
