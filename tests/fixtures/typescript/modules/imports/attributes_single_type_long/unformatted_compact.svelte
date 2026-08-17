<script lang="ts">
	/* lone `type` attribute - over print width, stays on one line */
	import a from './pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp.json' with {type:'json'};

	/* quoted `type` key - unquoted by quoteProps, same rule */
	import b from './pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp.json' with {type:'json'};

	/* re-export host - same rule */
	export { dd } from './pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp.json' with {type:'json'};

	/* export-all host - same rule */
	export * from './pppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppppp.json' with {type:'json'};

	/* control: a non-`type` key is an ordinary attribute list, so it breaks */
	import e from './rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr.json' with {resolution:'json'};

	/* control: a second attribute restores breaking */
	import f from './ssssssssssssssssssssssssssssssssssssssssssssssssss.json' with {type:'json',resolution:'import'};

	/* control: a comment on the attribute restores breaking */
	import g from './qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq.json' with {type:'json'/* c */};
</script>
