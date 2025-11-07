<style>
	:root {
		--var1: #007bff;
		--var2: #6c757d;
		--var3: 16px;
		--var4: 14px;
	}

	.class1 {
		background-color: var(--var1);
		padding: var(--var3);
		font-size: var(--var4);
	}

	.class2 {
		color: var(--var5, black);
		margin: var(--var6, 0);
	}

	.class3 {
		width: var(--var7, 100px);
		height: var(--var7, 100px);
	}
</style>
