<script lang="ts">
	var let = 1;
	var yield = 1;
	var static = 1;
	var package = 1;
	var implements = 1;
	var private = 1;
	var protected = 1;
	var public = 1;

	function fn_let(let) {}
	function fn_yield(yield) {}
	function fn_static(static) {}
	function fn_package(package) {}
	function fn_implements(implements) {}
	function fn_private(private) {}
	function fn_protected(protected) {}
	function fn_public(public) {}

	function named_let() {
		var v = function let() {};
	}

	class implements {}
	class C {
		constructor(private x) {}
	}
	enum yield {}
	type T = X extends Y ? infer let : never;
</script>
