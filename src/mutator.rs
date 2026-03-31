use std::borrow::Cow;
use std::num::NonZeroUsize;

use libafl::corpus::CorpusId;
use libafl::inputs::{BytesInput, HasMutatorBytes};
use libafl::mutators::MutationResult;
use libafl::state::HasRand;
use libafl::Error;
use libafl_bolts::rands::Rand;
use libafl_bolts::Named;

use crate::generator::{ExprGenerator, ALL_OPS};

fn rand_below(rng: &mut impl Rand, upper: usize) -> usize {
    if upper <= 1 {
        return 0;
    }
    rng.below(NonZeroUsize::new(upper).unwrap())
}

pub struct ExprMutator {
    generator: ExprGenerator,
}

impl ExprMutator {
    pub fn new() -> Self {
        Self {
            generator: ExprGenerator::new(3),
        }
    }

    /// Prepend or append a generated sub-expression with a random operator.
    fn compose(&self, input: &mut BytesInput, rng: &mut impl Rand) -> MutationResult {
        let existing = input.mutator_bytes().to_vec();
        if existing.is_empty() {
            return MutationResult::Skipped;
        }

        let new_expr = self.generator.random_expr(rng, 0).to_string();
        let op = ALL_OPS[rand_below(rng, ALL_OPS.len())];
        let existing_str = String::from_utf8_lossy(&existing);

        let combined = if rand_below(rng, 2) == 0 {
            format!("{new_expr}{op}{existing_str}")
        } else {
            format!("{existing_str}{op}{new_expr}")
        };

        *input = BytesInput::new(combined.into_bytes());
        MutationResult::Mutated
    }

    fn wrap_parens(&self, input: &mut BytesInput) -> MutationResult {
        let bytes = input.mutator_bytes();
        let mut new = Vec::with_capacity(bytes.len() + 2);
        new.push(b'(');
        new.extend_from_slice(bytes);
        new.push(b')');
        *input = BytesInput::new(new);
        MutationResult::Mutated
    }

    fn add_negation(&self, input: &mut BytesInput) -> MutationResult {
        let bytes = input.mutator_bytes();
        let mut new = Vec::with_capacity(bytes.len() + 1);
        new.push(b'-');
        new.extend_from_slice(bytes);
        *input = BytesInput::new(new);
        MutationResult::Mutated
    }

}

impl Named for ExprMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("ExprMutator");
        &NAME
    }
}

impl<S> libafl::mutators::Mutator<BytesInput, S> for ExprMutator
where
    S: HasRand,
{
    fn mutate(&mut self, state: &mut S, input: &mut BytesInput) -> Result<MutationResult, Error> {
        if input.mutator_bytes().len() > 100 {
            let expr = self.generator.random_expr(state.rand_mut(), 0);
            *input = BytesInput::new(expr.to_string().into_bytes());
            return Ok(MutationResult::Mutated);
        }

        let choice = rand_below(state.rand_mut(), 100);

        Ok(match choice {
            0..=59 => self.compose(input, state.rand_mut()),
            60..=79 => self.wrap_parens(input),
            _ => self.add_negation(input),
        })
    }

    fn post_exec(&mut self, _state: &mut S, _new_corpus_id: Option<CorpusId>) -> Result<(), Error> {
        Ok(())
    }
}
