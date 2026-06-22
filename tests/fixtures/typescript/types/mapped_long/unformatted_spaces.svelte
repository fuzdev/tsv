<script lang="ts">
	// Basic mapped type

	// 100 chars - stays inline (at boundary)
	type Inline = {  [  K   in   keyof   SomeTypeAAAAAAAAAAAAAAAAAAAAAAA  ]  :   SomeTypeAAAAAAAAAAAAAAAAAAAAAA  [  K  ]  };

	// 101 chars - wraps (exceeds print_width)
	type Wrapped = {
		[  K   in   keyof   SomeTypeAAAAAAAAAAAAAAAAAAAAAAA  ]  :   SomeTypeAAAAAAAAAAAAAAAAAAAAAA  [  K  ]  ;
	};

	// Modifiers

	// readonly modifier (100 chars - stays inline)
	type ReadonlyInline = {  readonly   [  K   in   keyof   SomeTypeAAAAAAAAAAAAA  ]  :   SomeTypeAAAAAAAAAAAAAAA  [  K  ]  };

	// readonly modifier (101 chars - wraps)
	type ReadonlyWrapped = {
		readonly   [  K   in   keyof   SomeTypeAAAAAAAAAAAAA  ]  :   SomeTypeAAAAAAAAAAAAAAA  [  K  ]  ;
	};

	// optional modifier (100 chars - stays inline)
	type OptionalInline = {  [  K   in   keyof   SomeTypeAAAAAAAAAAAAAAAAAAA  ]  ?  :   SomeTypeAAAAAAAAAAAAAAAAA  [  K  ]  };

	// optional modifier (101 chars - wraps)
	type OptionalWrapped = {
		[  K   in   keyof   SomeTypeAAAAAAAAAAAAAAAAAAA  ]  ?  :   SomeTypeAAAAAAAAAAAAAAAAA  [  K  ]  ;
	};

	// remove optional modifier (100 chars - stays inline)
	type RequiredInline = {  [  K   in   keyof   SomeTypeAAAAAAAAAAAAAAAAAA  ]  -  ?  :   SomeTypeAAAAAAAAAAAAAAAAA  [  K  ]  };

	// remove optional modifier (101 chars - wraps)
	type RequiredWrapped = {
		[  K   in   keyof   SomeTypeAAAAAAAAAAAAAAAAAA  ]  -  ?  :   SomeTypeAAAAAAAAAAAAAAAAA  [  K  ]  ;
	};

	// As clause for key remapping

	// as clause (stays inline)
	type RemapInline = {  [  K   in   keyof   T   as   K   extends   string   ?   K   :   never  ]  :   T  [  K  ]  };

	// as clause with conditional value (wraps due to complexity)
	type RemapWrapped = {
		[  K   in   keyof   T   as   K   extends   string   ?   K   :   never  ]  :   T  [  K  ]   extends   infer   U   ?   U   :   never  ;
	};

	// Complex mapped type with deeply nested conditional

	type DeepConditional = {
		[  K   in   keyof   T  ]  :   T  [  K  ]   extends   Array  <  infer   U  >
			?   U
			:   T  [  K  ]   extends   object
				?   DeepConditional  <  T  [  K  ]  >
				:   T  [  K  ]  ;
	};
</script>
