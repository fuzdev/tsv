<script lang="ts">
	// A hugged sole object-type parameter breaks BEFORE a breakable return type:
	// the object type prints with no group of its own, so once the signature
	// overflows its members expand while the return type's arguments stay inline.

	// Function: 100 chars stays inline, 101 expands the object
	function fn100(options  :  {  aaaa  :  Aaaaaaaaa  ;  bbb  :  Bbbbbb  })  :  Promise<Result<{  value  :  boolABCD  }>>  {}
	function fn101(options  :  {
		aaaa  :  Aaaaaaaaa;
		bbb  :  Bbbbbb;
	})  :  Promise<Result<{  value  :  boolABCDE  }>>  {}

	// Bodyless declaration
	declare function decl(options  :  {
		aaaa  :  Aaaaaaaaa;
		bbb  :  Bbbbbb;
	})  :  Promise<Result<{  value  :  boolean  }>>;

	// Arrow
	const arrow  =  (options  :  {
		aaaa  :  Aaaaaaaaa;
		bbb  :  Bbbbbb;
	})  :  Promise<Result<{  value  :  boolean  }>>   =>  {};

	// Class method and interface signatures
	class Single {
		method(options  :  {
			aaaa  :  Aaaaaaaaa;
			bbb  :  Bbbbbb;
		})  :  Promise<Result<{  value  :  booleanABCDEFGH  }>>  {}
	}
	interface Multiple {
		method(options  :  {
			aaaa  :  Aaaaaaaaa;
			bbb  :  Bbbbbb;
		})  :  Promise<Result<{  value  :  booleanABCDEFGHIJ  }>>;
		(options  :  {
			aaaa  :  Aaaaaaaaa;
			bbb  :  Bbbbbb;
		})  :  Promise<Result<{  value  :  booleanABCDEFGHIJKLMNOP  }>>;
	}

	// Function, constructor, and construct-signature types
	type A  =  (options  :  {
		aaaa  :  Aaaaaaaaa;
		bbb  :  Bbbbbb;
	})  =>  Promise<Result<{  value  :  booleanABCDEFG  }>>;
	type B  =  new  (options  :  {
		aaaa  :  Aaaaaaaaa;
		bbb  :  Bbbbbb;
	})  =>  Promise<Result<{  value  :  booleanABC  }>>;
	interface Construct {
		new  (options  :  {
			aaaa  :  Aaaaaaaaa;
			bbb  :  Bbbbbb;
		})  :  Promise<Result<{  value  :  booleanABCDEFGHIJKL  }>>;
	}
</script>
