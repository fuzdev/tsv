<script lang="ts">
	// short union assertion fits inline
	const a = <A|B>expr;

	// exactly 100 wide: union cast stays inline
	const b = <Map<string,number,boolean,object,null,undefined,never,symbol>|undefined1>expr;

	// 101 wide, type fits the `<type>(` line: the expression breaks into parens
	const c = <Map<string,number,boolean,object,null,undefined,never,symbol>|undefined12>expr;

	// type too long for the `<type>(` line: the cast breaks after `<`, union flat (no leading `|`)
	const d = <Map<string,boolean,number,object,null,undefined,any,void,never,symbol>|undefined>expr;

	// object-literal expr stays hugged after `>` even when the cast type breaks
	const e = <Map<string,number,boolean,object,null,undefined,never,symbol>|undefined>{prop:a};
</script>
