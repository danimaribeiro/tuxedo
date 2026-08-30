class Dog
  def speak
    puts "woof"
  end
end

class Animals::Cat < Base
  def speak(sound)
    puts sound
  end
end

class Zoo::Exotic::Bird
  def fly
    puts "flap"
  end
end
