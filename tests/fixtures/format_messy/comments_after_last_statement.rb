class LlmCreditAllocation < ActiveRecord::Base
  def relative_reset_time
    AgeWords.distance_of_time_in_words(Time.current, next_reset_at)
  end

  private
end

# == Schema Information
#
# Table name: llm_credit_allocations
