<script lang="ts">
	// A tuple label is an IdentifierName, so a keyword spells one too.
	type Reserved = [function: string, typeof: number, new: boolean, import: string];
	type Contextual = [string: number, any: string, yield: boolean, await: number];
	type Primitive = [this: string, void: number, null: boolean, true: string];
	type Optional = [function?: string, this?: number, string?: boolean];
	type Rest = [first: string, ...function: number[]];
	// Without the `:` the same keyword head is an ordinary type, postfix `?` and all.
	type NotLabels = [function, this?, void?];
</script>
