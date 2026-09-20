x = [1, 2, 3]
[
  foo { bar },
  element_number_0,
  element_number_1,
  element_number_2,
  element_number_3,
  element_number_4,
  element_number_5
]
[
  element_number_0,
  element_number_1,
  element_number_2,
  element_number_3,
  element_number_4,
  element_number_5,
  element_number_6,
  element_number_7,
  element_number_8,
  element_number_9
]

# A bare keyword hash in an array is one element: when the array expands,
# every pair sits at the element indent.
trace = [
  1,
  exception_object_id: exception.object_id,
  id: 0,
  trace: wrapper.source_extracts.first[:trace],
  more: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
]

s.files = Dir[
  "CHANGELOG.md",
  "MIT-LICENSE",
  "README.md",
  "lib/**/*",
  "app/**/*",
  "config/**/*",
  "db/**/*",
  "package.json"
]
x = a[1, 2]
