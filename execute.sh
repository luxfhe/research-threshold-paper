output_dir="benchmark_results"
mkdir -p "$output_dir"

run_test() {
    local use_tc="$1"  # Whether to use tc or not
    local delay="$2"
    local bandwidth="$3"
    local n="$4"
    local m="$5"
    local b="$6"


    if [[ "$use_tc" == "with_tc" ]]; then
        echo "Setting delay: $delay and bandwidth: $bandwidth"
#        sudo tc qdisc add dev lo root netem delay "$delay" rate "$bandwidth"
    fi


    echo "Running discovery-server with n=$n, m=$m, b=$b ($use_tc)"
    cargo run -r --package threshold-decryption --bin network -- -n "$n" -k "$k" -m "$m" -b "$b" --lwe-bits "$lwe_bits" --mac-s "$mac_s" discovery-server &
    sleep 1

    for i in $(seq 0 $((n - 1))); do
        echo "Running participant $i with n=$n, m=$m, b=$b ($use_tc)"
        local output_file="$output_dir/delay_${delay}_bw_${bandwidth}_n_${n}_k_${k}_m_${m}_b_${b}_party${i}.txt"
        cargo run -r --package threshold-decryption --bin network -- -n "$n" -k "$k" -m "$m" -b "$b" --lwe-bits "$lwe_bits" --mac-s "$mac_s" participant "$i" > "$output_file" &
        sleep 0.1
    done
    wait

    if [[ "$use_tc" == "with_tc" ]]; then
        # Clean up tc settings after the run
        echo "Cleaning up tc settings"
#        sudo tc qdisc del dev lo root
    fi
}

delay="0.5ms"
bandwidth="10mbit"
n=4

m=1
b=8
k=64
mac_s=64
lwe_bits=1024

sudo lsof -i :5000 | awk 'NR>1 {print $2}' | xargs sudo kill -9
sudo tc qdisc del dev lo root
# run_test "no_tc" "no" "no"

run_test "with_tc" "$delay" "$bandwidth" "$n" "$m" "$b"

# sudo lsof -i :5000 | awk 'NR>1 {print $2}' | xargs sudo kill -9
# sudo tc qdisc del dev lo root
# sudo tc qdisc add dev lo root netem delay 1ms rate 100mbit
