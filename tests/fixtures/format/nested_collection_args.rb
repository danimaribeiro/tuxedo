foo(
  a,
  [
    element_number_0,
    element_number_1,
    element_number_2,
    element_number_3,
    element_number_4,
    element_number_5,
    element_number_6,
    element_number_7
  ],
  b
)

foo(
  x,
  bar(
    argument_number_0,
    argument_number_1,
    argument_number_2,
    argument_number_3,
    argument_number_4,
    argument_number_5,
    argument_number_6
  ),
  y
)

foo(
  a,
  {
    key_number_0: 0,
    key_number_1: 1,
    key_number_2: 2,
    key_number_3: 3,
    key_number_4: 4,
    key_number_5: 5,
    key_number_6: 6
  }
)

def example
  verifier_sha512 = ActiveSupport::MessageVerifier.new(
    app.key_generator.generate_key("sha512 salt"),
    digest: :SHA512,
    serializer: Marshal
  )
end

configure(
  name: "x",
  enabled: true,
  retries: 5,
  extra_long_option_name: true,
  another_long_option: false,
  third_long_option: 1
)

VALID_UNSCOPING_VALUES = Set
  .new(
    [
      :where,
      :select,
      :group,
      :order,
      :lock,
      :readonly,
      :having,
      :offset,
      :joins,
      :left_outer_joins,
      :annotate,
      :optimizer_hints,
      :from
    ]
  )
  .freeze
