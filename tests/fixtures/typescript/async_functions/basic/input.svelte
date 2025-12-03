<script>
	// Async function declaration
	async function fetchData() {
		return await fetch('/api');
	}

	// Async arrow function
	const fn = async () => {
		await doSomething();
	};

	// Async arrow with params
	const fetchUser = async (id) => {
		const user = await getUser(id);
		return user;
	};

	// Async method shorthand (in object)
	const obj = {
		async getData() {
			return await this.load();
		},
	};

	// Await expression
	const result = await promise;

	// Await with member expression
	const data = await obj.method();

	// Await with call expression
	const value = await getValue();

	// Nested await
	const nested = await (await getPromise()).value;
</script>
