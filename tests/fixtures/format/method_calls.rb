user.name
user.name = "Ada"
user.save(true)
list[0]
list[0] = "first"
list[0, 1] = "a"
user&.name
user.valid?
desc = +""
x = -y
z = ~w
q = !r

short.chain.of.calls

receive(:readline).with("y").and_return("y")

expect(Thor::LineEditor).to receive(:readline)
  .with("Should I overwrite it? ", { add_to_history: false })
  .and_return("y")

expect(Thor::LineEditor).to receive(:readline)
  .with(
    "What's your favorite Neopolitan flavor? [strawberry, chocolate, vanilla] ",
    { limited_to: flavors, case_insensitive: true }
  )
  .and_return("CHOCOLATE")
