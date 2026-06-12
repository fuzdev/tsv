<script lang="ts">
	// Object type + null - should hug (inline object, null appended)
	type Result =
		| {
				name: string;
				value: number;
		  }
		| null;

	// Object type + void - should hug
	type Nullable =
		| {
				data: string;
		  }
		| void;

	// Object type + null + void - should hug (all non-object members are void types)
	type Mixed =
		| {
				id: number;
				label: string;
		  }
		| null
		| void;

	// Type reference + null - should hug (TSTypeReference is object-like)
	type Ref = Map<string, number> | null;

	// Object type + boolean - should NOT hug (boolean is not a void type)
	type NotHugged =
		| {
				prop: string;
		  }
		| boolean;

	// Two object types - should NOT hug (more than one object-like type)
	type TwoObjects =
		| {
				a: string;
		  }
		| {
				b: number;
		  };

	// Long object type + null - should hug even when object breaks to multiline
	type LongResult =
		| {
				name: string;
				description: string;
				category: string;
		  }
		| null;
</script>
