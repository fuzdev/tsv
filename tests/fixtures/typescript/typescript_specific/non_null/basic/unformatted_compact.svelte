<script lang="ts">
//
// Non-Null Assertion Operator (!)
//

// Basic non-null assertion
a!;

// Member access after non-null
a!.b;
a!.b!.c!;

// Call result
fn()!;
a.fn()!.b;

// Array/index access
a[0]!;
a.get('key')!;

// Optional chaining then non-null
a?.b!;
a?.b?.c!;

// Parenthesized (low precedence expressions)
(a||b)!;
(a+b)!;
(a?b:c)!;
(a,b)!;

// With await
(await promise)!;
(await a.fn())!.b;

// In assignment
const x=a!;
const y=a!.b!;

// With type assertion
(a as T)!;
(<T>a)!;

// Double assertion (non-null twice - valid but unusual)
a!!;

// Non-null in expression context
fn(a!,b!);
a!+b!;
a!?b:c;
</script>
