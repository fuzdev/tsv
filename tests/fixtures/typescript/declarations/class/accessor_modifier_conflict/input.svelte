<script lang="ts">
	class C {
		// a get/set accessor is neither a generator nor an async method — each is fine alone
		get a() {
			return 1;
		}
		set a(v) {}
		// `async` and `*` combine only with each other (an async generator method)
		async *b() {}
		// `async`/`*` prefixing a method NAMED get/set commits no accessor kind — stays valid
		async get() {}
		*set() {}
	}
</script>
