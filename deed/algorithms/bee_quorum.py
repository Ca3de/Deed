"""
Bee Quorum Consensus for distributed decision-making.

Inspired by honeybee swarm intelligence, this module implements
fast consensus reaching through waggle dance-like communication
and quorum sensing.

Key principles:
- Multiple scouts evaluate different options independently
- Quality determines "dance intensity" (advertising strength)
- Quorum threshold triggers consensus
- No central coordinator needed
"""

from typing import Dict, List, Any, Optional, Set
from dataclasses import dataclass, field
from datetime import datetime
import random


@dataclass
class ScoutBee:
    """
    A scout bee that evaluates and advertises one option.

    In database context, this could be:
    - A candidate query plan
    - A replica to use for reads
    - A leader candidate in distributed consensus
    """

    scout_id: int
    option_id: str  # ID of the option being scouted
    option_data: Dict[str, Any]  # The actual option data

    # Quality assessment
    quality_score: float = 0.0  # 0-1, higher is better

    # Communication
    dance_intensity: float = 0.0  # How vigorously to advertise (0-1)
    supporters: Set[int] = field(default_factory=set)  # IDs of scouts supporting this
    stop_signals_received: int = 0  # Cross-inhibition from other scouts

    # Metadata
    discovered_at: datetime = field(default_factory=datetime.now)

    def evaluate_quality(self, evaluation_context: Dict[str, Any]) -> float:
        """
        Assess the quality of this option.

        Args:
            evaluation_context: Information needed to evaluate quality

        Returns:
            Quality score (0-1, higher is better)

        In real implementation, this might:
        - Test query performance
        - Check replica latency
        - Evaluate node health metrics
        """
        # Simplified quality evaluation
        # Real implementation would perform actual tests

        # Base quality from option metadata
        base_quality = self.option_data.get('estimated_quality', 0.5)

        # Adjust based on context (e.g., current load, network conditions)
        load_factor = 1.0 - evaluation_context.get('load', 0.0)

        quality = base_quality * load_factor

        # Add small random variation (simulating real-world variability)
        quality *= random.uniform(0.9, 1.1)

        self.quality_score = max(0.0, min(1.0, quality))
        return self.quality_score

    def calculate_dance_intensity(self) -> float:
        """
        Determine how strongly to advertise this option.

        Biological: Better nest sites get longer, more vigorous waggle dances.
        Database: Better plans/replicas get stronger recommendation signals.
        """
        # Dance intensity proportional to quality
        intensity = self.quality_score

        # Reduce if received stop signals (cross-inhibition)
        inhibition = self.stop_signals_received * 0.1
        intensity = max(0.0, intensity - inhibition)

        self.dance_intensity = intensity
        return intensity

    def recruit_supporter(self, scout_id: int) -> None:
        """Another scout has been convinced to support this option."""
        self.supporters.add(scout_id)

    def receive_stop_signal(self) -> None:
        """
        Receive a stop signal from a scout advertising a different option.

        This is cross-inhibition - helps narrow choices quickly.
        """
        self.stop_signals_received += 1

    def get_support_count(self) -> int:
        """Number of scouts supporting this option (including self)."""
        return len(self.supporters) + 1  # +1 for self


class BeeQuorumConsensus:
    """
    Distributed consensus using honeybee quorum sensing.

    Faster and more robust than traditional voting protocols for
    certain decision types. Especially good for:
    - Choosing among multiple good options (not binary yes/no)
    - When stragglers/failures shouldn't block decisions
    - When quality varies and should influence outcome
    """

    def __init__(
        self,
        quorum_threshold: int = 15,
        max_rounds: int = 10,
        cross_inhibition_enabled: bool = True
    ):
        """
        Initialize bee quorum consensus.

        Args:
            quorum_threshold: Number of supporters needed for consensus
            max_rounds: Maximum decision rounds before fallback
            cross_inhibition_enabled: Whether scouts send stop signals
        """
        self.quorum_threshold = quorum_threshold
        self.max_rounds = max_rounds
        self.cross_inhibition_enabled = cross_inhibition_enabled

        self.stats = {
            'total_decisions': 0,
            'avg_rounds_to_consensus': 0,
            'quorum_reached_count': 0,
            'fallback_count': 0,
        }

    def reach_consensus(
        self,
        options: List[Dict[str, Any]],
        num_scouts: int,
        evaluation_context: Dict[str, Any]
    ) -> Optional[Dict[str, Any]]:
        """
        Reach consensus on best option among candidates.

        Args:
            options: List of candidate options to choose from
            num_scouts: Number of scout bees to deploy
            evaluation_context: Context for quality evaluation

        Returns:
            Chosen option, or None if no consensus possible

        Process:
        1. Assign scouts to evaluate options
        2. Scouts assess quality and "dance" (advertise)
        3. Scouts recruit each other based on dance intensity
        4. Apply cross-inhibition (stop signals)
        5. Check for quorum
        6. Repeat until quorum reached or max rounds

        Example:
            options = [
                {'id': 'plan_a', 'estimated_quality': 0.7, ...},
                {'id': 'plan_b', 'estimated_quality': 0.9, ...},
            ]
            consensus.reach_consensus(options, num_scouts=20, context={})
        """
        if not options:
            return None

        # Initialize scouts
        scouts: List[ScoutBee] = []

        # Assign scouts to options (weighted by initial quality estimates)
        for i in range(num_scouts):
            # Choose option to scout (initially random, favor high estimates)
            if random.random() < 0.3 and options:
                # Random exploration
                option = random.choice(options)
            else:
                # Favor options with high estimated quality
                options_with_weights = [
                    (opt, opt.get('estimated_quality', 0.5))
                    for opt in options
                ]
                total_weight = sum(w for _, w in options_with_weights)
                if total_weight > 0:
                    rand = random.uniform(0, total_weight)
                    cumulative = 0
                    option = options[0]
                    for opt, weight in options_with_weights:
                        cumulative += weight
                        if rand <= cumulative:
                            option = opt
                            break
                else:
                    option = random.choice(options)

            scout = ScoutBee(
                scout_id=i,
                option_id=option['id'],
                option_data=option
            )
            scout.evaluate_quality(evaluation_context)
            scouts.append(scout)

        # Iterative rounds of communication and recruitment
        decision_rounds = 0

        for round_num in range(self.max_rounds):
            decision_rounds += 1

            # Phase 1: All scouts calculate dance intensity
            for scout in scouts:
                scout.calculate_dance_intensity()

            # Phase 2: Recruitment - scouts observe dances and may switch support
            for observer in scouts:
                # Each scout observes some other scouts' dances
                sample_size = min(5, len(scouts) - 1)
                observed = random.sample(
                    [s for s in scouts if s.scout_id != observer.scout_id],
                    sample_size
                )

                # Find most impressive dance
                best_observed = max(observed, key=lambda s: s.dance_intensity)

                # Switch support if observed dance is significantly better
                if best_observed.dance_intensity > observer.dance_intensity * 1.2:
                    # Recruit: switch to supporting the better option
                    observer.option_id = best_observed.option_id
                    observer.option_data = best_observed.option_data
                    observer.quality_score = best_observed.quality_score
                    best_observed.recruit_supporter(observer.scout_id)

            # Phase 3: Cross-inhibition (optional)
            if self.cross_inhibition_enabled:
                # Group scouts by option
                option_groups: Dict[str, List[ScoutBee]] = {}
                for scout in scouts:
                    if scout.option_id not in option_groups:
                        option_groups[scout.option_id] = []
                    option_groups[scout.option_id].append(scout)

                # Scouts in larger groups send stop signals to smaller groups
                for option_id, group in option_groups.items():
                    # Send stop signals to scouts in other groups
                    for scout in scouts:
                        if scout.option_id != option_id:
                            if random.random() < 0.3:  # Probabilistic
                                scout.receive_stop_signal()

            # Phase 4: Check for quorum
            # Count supporters for each option
            option_support: Dict[str, int] = {}
            for scout in scouts:
                if scout.option_id not in option_support:
                    option_support[scout.option_id] = 0
                option_support[scout.option_id] += 1

            # Check if any option reached quorum
            for option_id, support_count in option_support.items():
                if support_count >= self.quorum_threshold:
                    # Quorum reached!
                    self.stats['quorum_reached_count'] += 1
                    self.stats['total_decisions'] += 1

                    # Update average rounds
                    n = self.stats['quorum_reached_count']
                    current_avg = self.stats['avg_rounds_to_consensus']
                    self.stats['avg_rounds_to_consensus'] = (
                        (current_avg * (n - 1) + decision_rounds) / n
                    )

                    # Find and return the chosen option
                    for scout in scouts:
                        if scout.option_id == option_id:
                            return scout.option_data

        # No quorum reached - fallback to best quality option
        self.stats['fallback_count'] += 1
        self.stats['total_decisions'] += 1

        # Return option with highest quality among scouts
        best_scout = max(scouts, key=lambda s: s.quality_score)
        return best_scout.option_data

    def get_stats(self) -> Dict[str, Any]:
        """Get consensus statistics."""
        return {
            **self.stats,
            'quorum_success_rate': (
                self.stats['quorum_reached_count'] / self.stats['total_decisions']
                if self.stats['total_decisions'] > 0
                else 0
            ),
        }

    def __repr__(self) -> str:
        return (
            f"BeeQuorumConsensus("
            f"quorum={self.quorum_threshold}, "
            f"decisions={self.stats['total_decisions']}, "
            f"avg_rounds={self.stats['avg_rounds_to_consensus']:.1f})"
        )
