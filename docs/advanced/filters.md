# Filtering
GitHub Backup includes a comprehensive filtering language which allows you
to specify exactly which repositories you want to backup. This filtering is
designed to be intuitive and easy to use in novel ways.

At its heart, a filter is just a boolean expression which evaluates to a truthy
value when you want to backup a repository, and a falsey value when you do not.
These values are usually based on the properties of the entity being evaluated,
whether that be a GitHub repository, a release, or a release artifact.

## Examples
Here are a few common filter examples which you might use in your configuration.

- `!repo.fork || !repo.archived || !repo.empty` - Do not include repositories which are forks, archived, or empty.
- `repo.private` - Only include private repositories in your list.
- `repo.public && !repo.fork` - Only include public repositories which are not forks.
- `repo.name contains "awesome"` - Only include repositories which have "awesome" in their name.
- `(repo.name contains "awesome" || repo.name contains "cool") && !repo.fork` - Only include repositories which have "awesome" or "cool" in their name and are not forks.
- `!release.prerelease && !asset.source-code` - Only include release artifacts which are not marked as pre-releases and are not source code archives.
- `repo.name in ["git-tool", "grey"]` - Only include repositories with the names "git-tool" or "grey".
- `repo.stargazers >= 5` - Only include repositories with at least 5 stars.

## Language Features
### Properties - `repo.<field>`
Accessing a property of the entity being evaluated is done using the `repo.<field>` syntax. This allows you
to access properties such as the repository's name, whether it is a fork, or how many stars it has.

The available properties depend on the entity being evaluated, with repositories supporting a different set of
properties to releases and release artifacts. You can find more information about [`repo`](../reference/repo.md),
[`release`](../reference/release.md), and [`asset`](../reference/release.md) properties in their respective reference
sections.

### Grouping - `( ... )`
The grouping operator allows you to group expressions together, ensuring that they are evaluated as a single
unit. This is most commonly used to combine multiple subexpressions into a single larger filter. For example,
`(repo.name contains "awesome" || repo.name contains "cool") && !repo.fork`.

### Literals
#### Strings
Strings are specified using double quotes (`"`), for example `"awesome"`. You can escape double quotes within a
string using a backslash (`\`), for example `"\"awesome\""`. Strings are case sensitive and empty strings are considered
falsey when evaluated.

::: tip
If you wish to treat an empty string as a valid value, you can use `repo.<field> != null` which will avoid the falsey
evaluation of an empty string.
:::

#### Numbers
Numbers are represented internally as a 64-bit floating-point value, which means that they can represent most reasonably sized
integers as well as most reasonably precise decimal numbers. For example, `5` and `5.0` are equivalent in the filter language.

You may specify negative numbers by prefixing them with a `-`, for example `-5`, and the number `0` is considered falsey when
evaluated.

#### Booleans
Booleans are represented as `true` and `false` in the filter language, and are used to represent the truthiness of a value. For
example, `repo.fork` will evaluate to `true` if the repository is a fork, and `false` if it is not.

#### Null/Undefined
The `null` value is used to represent the absence of a value, and is considered falsey when evaluated. Accessing a property which
does not exist will return `null`.

## Operators
### Unary Negation - `!`
The unary negation operator converts the following expression into the boolean opposite of its value.
For truthy values, this will return `false`, and for falsey values, it will return `true`.

It is most commonly used in cases where you would like to exclude forks, or empty repositories, from
being backed up.

::: tip
The terms "truthy" and "falsey" refer to values whose logical interpretation is `true` or `false` but
whose literal value may be different. For example, the number `0` is falsey, while the number `1` is
truthy.

We consider `null`, `0`, `false`, `""`, and `[]` (an empty array/tuple) to be falsey values, while all other values
are considered truthy.
:::

### Logical OR - `||`
The logical OR operator evaluates to the the value of the left hand expression if the left hand expression's
value is truthy, otherwise it evaluates to the value of the right hand expression. In practice this means
that it will return a truthy value if either the left or right hand expression is truthy, and will return
a falsey value only if both are falsey.

It is most commonly used to indicate that you would like to backup repositories which match one of several
conditions, for example: `repo.name contains "awesome" || repo.name contains "cool"`.

### Logical AND - `&&`
The logical AND operator evaluates to the value of the right hand expression if the left hand expression's
value is truthy, otherwise it evaluates to the value of the left hand expression. In practice this means
that it will return a truthy value if both the left and right hand expressions are truthy, otherwise a falsey
value will be returned.

It is most commonly used to indicate that you would like to backup repositories which match multiple conditions,
for example: `repo.name contains "awesome" && !repo.fork`.

### Comparison Operators - `==`, `!=`, `>`, `<`, `>=`, `<=`
The comparison operators are used to compare two values and return a boolean value indicating the result of the
comparison. These operators **DO NOT** perform type coercion, which means that you must compare values of the same
type - for example, comparing `5 <= "5" || 5 >= "5"` will always return `false`.

::: warning
String comparisons are performed using a case-insensitive comparison of ASCII characters, which means that `"Hello" == "hello"` will return `true`,
as will `"hello👋" == "hello"`.
:::

 - `==` - Returns `true` if the left and right hand expressions are equal.
 - `!=` - Returns `true` if the left and right hand expressions are not exactly equal.
 - `>` - Returns `true` if the left hand expression is strictly greater than the right hand expression.
 - `<` - Returns `true` if the left hand expression is strictly less than the right hand expression.
 - `>=` - Returns `true` if the left hand expression is greater than or equal to the right hand expression.
 - `<=` - Returns `true` if the left hand expression is less than or equal to the right hand expression.

::: tip
When comparing arrays/tuples, the comparison is performed element-wise, with the first element being compared between both arrays, then the second, and so on.
As such, `[1, 2, 3] > [1, 2, 2]` will return `true`. In cases where the arrays are of different lengths, the shorter array is considered to be less than the longer array.
:::

### Membership Operators - `in`, `contains`
The membership operators are used to check if a value is present within another value. The `in` operator is used to
determine whether the left hand value appears within the right; while the `contains` operator is used to determine
whether the right hand value appears within the left.

 - `x in y` - Returns `true` if `x` appears within `y`.
 - `y contains x` - Returns `true` if `x` appears within `y`.

These operators can be applied to both strings and arrays/tuples, with strings being searched for a matching substring
and arrays/tuples being searched for a matching element.


### Prefix and Suffix Matching - `startswith`, `endswith`
The prefix and suffix matching operators are used to determine whether a string starts or ends with a specific substring.

 - `"hello" startswith "he"` - Determines whether the string `hello` starts with the sequence `he`, returning `true` in this case.
 - `"goodbye" endswith "bye"` - Determines whether the string `goodbye` ends with the sequence `bye`, returning `true` in this case.

## Nerdy Details
The filtering language itself is implemented as a simple recursive descent parser which compiles an expression
tree from the input string. This expression tree is then evaluated using an interpreter to determine whether
a filter expression matches or not. The language is designed explicitly to avoid
[Turing completeness](https://en.wikipedia.org/wiki/Turing_completeness) as we require halting behaviour to
ensure that filters can be successfully evaluated. The language also eschews any means of inducing an error
or side-effect, ensuring that filters are safe to evaluate against untrusted data.

Behind the scenes, we've also worked hard to ensure that the evaluation of filters is performed with minimal
allocations, making it extremely fast. All of this works together to make the filtering language both more
ergonomic, easier to read, and safer than regular expressions and more powerful languages.
