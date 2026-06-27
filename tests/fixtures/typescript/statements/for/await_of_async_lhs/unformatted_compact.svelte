<script>
async function fn(){
// `async` is a plain IdentifierReference LHS — valid only in for-await
for await(async of [7]);

// A member-expression LHS starting with `async` is unaffected (the
// restriction is the exact two-token `async of`), with or without await
for await(async.x of y);
for(async.x of y);
}
</script>
