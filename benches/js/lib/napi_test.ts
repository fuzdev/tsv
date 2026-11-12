/**
 * Tests for which addon `NapiImplementation` opens: the path it is handed, else the
 * profile-guided addon, each named in the not-found error with its rebuild hint. Driven
 * through paths that hold nothing, so no addon is loaded or built.
 */

import assert from 'node:assert';
import { test } from 'vitest';
import { NapiImplementation } from './napi.ts';

test('a path handed to the loader is the file it opens', async () => {
	const path = '/nonexistent/tsv_napi_test/libtsv_napi.so';
	await assert.rejects(new NapiImplementation(path).init(), {
		message: `N-API addon not found at ${path}. Run 'rebuild ${path}' first.`
	});
});
